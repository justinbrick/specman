use rmcp::model::ErrorData;
use specman::SpecmanError;

pub type McpError = ErrorData;

pub fn to_mcp_error(err: SpecmanError) -> McpError {
    ErrorData::internal_error(err.to_string(), None)
}

pub fn invalid_params(message: impl Into<String>) -> McpError {
    ErrorData::invalid_params(message.into(), None)
}
