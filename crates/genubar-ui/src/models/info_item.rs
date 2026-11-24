/// Key-value field displayed inside the package details modal dialog.
#[derive(Debug, Clone)]
pub struct InfoField {
    pub label: String,
    pub value: String,
}

impl InfoField {
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
        }
    }
}
