use serde::Deserialize;
use super::Operation;

#[derive(Debug, Clone, Deserialize)]
pub struct Copy {
    src: String,
    dst: String,
}

impl Operation for Copy {

}
