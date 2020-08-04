use serde::Deserialize;
use super::Operation;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Copy {
    src: String,
    dst: String,
}

impl Operation for Copy {

}
