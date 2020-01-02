//! A client which caches repeated requests.

use super::GenericClient;
use crate::error::Error;
use async_trait::async_trait;
use futures::lock::Mutex;
use postgres_types::ToSql;
use std::collections::HashMap;
use std::hash::Hash;
use std::mem;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use tokio_postgres::{error::Error as SqlError, RowStream, Statement};

/// A wrapper which caches statements prepared through the [`GenericClient::prepare_static`] and [`GenericClient::prepare_static`] method.
///
/// [`GenericClient::prepare_static`]: trait.GenericClient#method.prepare_static
pub struct Cached<C>
where
    C: GenericClient,
{
    client: C,
    cache: Cache,
}

#[derive(Clone)]
pub struct QueryCache(Cache);

type Cache = Arc<Mutex<DynamicCache<StrKey, Statement>>>;

// We uniquely identify a `&'static str` using a pointer and a length.
// Since shared references with static lifetimes are guaranteed not to change we can assert that two
// `&'static str`s that point to the same value in fact are the same value during the whole duration
// of the program.
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
struct StrKey {
    ptr: usize,
    len: usize,
}

/// A cache optimized for a small number of items.
#[derive(Debug, Clone, PartialEq, Eq)]
enum DynamicCache<K, V>
where
    K: DynamicKey,
{
    Linear(Vec<(K, V)>),
    Hash(HashMap<K, V>),
}

/// A key with a dynamic cutoff.
trait DynamicKey: Hash + Eq {
    /// Maximum number of items in a linear search.
    const LINEAR_CUTOFF: usize;
}

impl<C> Cached<C>
where
    C: GenericClient,
{
    /// Wrap a client in a new cache.
    pub fn new(client: C) -> Cached<C> {
        Cached {
            client,
            cache: Cache::default(),
        }
    }
}

impl<C> From<C> for Cached<C>
where
    C: GenericClient,
{
    fn from(client: C) -> Self {
        Cached::new(client)
    }
}

impl<C> Deref for Cached<C>
where
    C: GenericClient,
{
    type Target = C;

    fn deref(&self) -> &Self::Target {
        &self.client
    }
}

impl<C> DerefMut for Cached<C>
where
    C: GenericClient,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.client
    }
}

#[async_trait]
impl<C> GenericClient for Cached<C>
where
    C: GenericClient + Sync + Send,
{
    async fn prepare(&self, sql: &str) -> Result<Statement, SqlError> {
        self.client.prepare(sql).await
    }

    async fn prepare_static(&self, sql: &'static str) -> Result<Statement, SqlError> {
        if let Some(statement) = self.get_cached(sql).await {
            Ok(statement)
        } else {
            let statement = self.client.prepare_static(sql).await?;
            self.cache(sql, statement.clone()).await;
            Ok(statement)
        }
    }

    async fn execute_raw<'a>(
        &'a self,
        statement: &Statement,
        parameters: &[&'a (dyn ToSql + Sync)],
    ) -> Result<u64, SqlError> {
        self.client.execute_raw(statement, parameters).await
    }

    async fn query_raw<'a>(
        &'a self,
        statement: &Statement,
        parameters: &[&'a (dyn ToSql + Sync)],
    ) -> Result<RowStream, SqlError> {
        self.client.query_raw(statement, parameters).await
    }
}

impl<C> Cached<C>
where
    C: GenericClient,
{
    async fn get_cached(&self, sql: &'static str) -> Option<Statement> {
        let cache = self.cache.lock().await;
        cache.get(&StrKey::new(sql)).map(Statement::clone)
    }

    async fn cache(&self, sql: &'static str, statement: Statement) {
        let mut cache = self.cache.lock().await;
        cache.insert(StrKey::new(sql), statement);
    }
}

impl StrKey {
    pub fn new(text: &'static str) -> StrKey {
        StrKey {
            ptr: text.as_ptr() as usize,
            len: text.len(),
        }
    }
}

impl DynamicKey for StrKey {
    // TODO: run benchmarks to find a good cutoff.
    const LINEAR_CUTOFF: usize = 64;
}

impl<K, V> DynamicCache<K, V>
where
    K: DynamicKey,
{
    pub fn get(&self, index: &K) -> Option<&V> {
        match self {
            DynamicCache::Linear(pairs) => pairs
                .iter()
                .find(|(key, _)| K::eq(key, &index))
                .map(|(_, value)| value),
            DynamicCache::Hash(map) => map.get(index),
        }
    }

    /// Insert a new key-value pair into the cache, and grow the cache if necessary.
    pub fn insert(&mut self, key: K, value: V) {
        match self {
            DynamicCache::Linear(pairs) if pairs.len() >= K::LINEAR_CUTOFF => {
                let map = mem::take(pairs).into_iter().collect();
                *self = DynamicCache::Hash(map);
                self.insert(key, value);
            }
            DynamicCache::Linear(pairs) => {
                pairs.push((key, value));
            }
            DynamicCache::Hash(map) => {
                map.insert(key, value);
            }
        }
    }
}

impl<K, V> Default for DynamicCache<K, V>
where
    K: DynamicKey,
{
    fn default() -> Self {
        DynamicCache::Linear(Vec::new())
    }
}

// TODO: Unfortunately we require GATs to do this in a more general fashion without resorting to
// dynamic dispatch. When GATs become stable we can move this into the `GenericClient` trait.
macro_rules! impl_cached_transaction {
    ($client:ty, $transaction:ty) => {
        impl Cached<$client> {
            /// Start a new transaction that shares the same cache as the current client.
            pub async fn transaction(&mut self) -> Result<Cached<$transaction>, Error> {
                <$client>::transaction(self)
                    .await
                    .map(Cached::new)
                    .map_err(Error::BeginTransaction)
            }
        }
    };
}

impl_cached_transaction!(tokio_postgres::Client, tokio_postgres::Transaction<'_>);
impl_cached_transaction!(
    tokio_postgres::Transaction<'_>,
    tokio_postgres::Transaction<'_>
);
