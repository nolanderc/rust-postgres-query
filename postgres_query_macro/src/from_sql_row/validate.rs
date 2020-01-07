use super::{ContainerAttributes, PartitionKind, Property};
use syn::Result;

pub(super) fn validate_properties(
    container: &ContainerAttributes,
    props: &[Property],
) -> Result<()> {
    check_split_in_non_split_container(container, props)?;
    check_stride_in_non_exact_container(container, props)?;

    Ok(())
}

fn check_split_in_non_split_container(
    container: &ContainerAttributes,
    props: &[Property],
) -> Result<()> {
    let is_split = is_match!(
        container.partition.as_ref().map(|attr| &attr.value),
        Some(PartitionKind::Split(_))
    );

    if is_split {
        Ok(())
    } else {
        let split = props
            .iter()
            .flat_map(|prop| prop.attrs.splits.iter())
            .next();

        match split {
            None => Ok(()),
            Some(split) => Err(err!(
                split.span,
                "explicit `split` in a container without the `#[row(split)]` attribute"
            )),
        }
    }
}

fn check_stride_in_non_exact_container(
    container: &ContainerAttributes,
    props: &[Property],
) -> Result<()> {
    let is_exact = is_match!(
        container.partition.as_ref().map(|attr| &attr.value),
        Some(PartitionKind::Exact)
    );

    if is_exact {
        Ok(())
    } else {
        let stride = props
            .iter()
            .filter_map(|prop| prop.attrs.stride)
            .next();

        match stride {
            None => Ok(()),
            Some(stride) => Err(err!(
                stride.span,
                "explicit `stride` in a container without the `#[row(exact)]` attribute"
            )),
        }
    }
}
