//! OData query options support.
//!
//! This module provides parsing and handling for OData system query options
//! including $select, $filter, $orderby, $top, $skip, $count, and $expand.

use rocket::form::FromForm;
use serde::Serialize;

/// OData system query options
#[derive(FromForm, Debug, Clone)]
pub struct ODataQuery {
    /// $select - comma-separated list of properties to include
    #[field(name = "$select")]
    pub select: Option<String>,
    
    /// $filter - filter expression
    #[field(name = "$filter")]
    pub filter: Option<String>,
    
    /// $orderby - comma-separated list of properties to order by
    #[field(name = "$orderby")]
    pub orderby: Option<String>,
    
    /// $top - maximum number of entities to return
    #[field(name = "$top")]
    pub top: Option<i64>,
    
    /// $skip - number of entities to skip
    #[field(name = "$skip")]
    pub skip: Option<i64>,
    
    /// $count - whether to include count of matching entities
    #[field(name = "$count")]
    pub count: Option<bool>,
    
    /// $expand - comma-separated list of navigation properties to expand
    #[field(name = "$expand")]
    pub expand: Option<String>,
}

impl Default for ODataQuery {
    fn default() -> Self {
        Self {
            select: None,
            filter: None,
            orderby: None,
            top: None,
            skip: None,
            count: None,
            expand: None,
        }
    }
}

impl ODataQuery {
    /// Parse $select into a list of property names
    pub fn parse_select(&self) -> Option<Vec<String>> {
        self.select.as_ref().map(|s| {
            s.split(',')
                .map(|prop| prop.trim().to_string())
                .collect()
        })
    }
    
    /// Parse $orderby into a list of property names with direction
    pub fn parse_orderby(&self) -> Option<Vec<(String, OrderDirection)>> {
        self.orderby.as_ref().map(|s| {
            s.split(',')
                .map(|item| {
                    let parts: Vec<&str> = item.trim().split_whitespace().collect();
                    if parts.len() >= 2 && parts[1].eq_ignore_ascii_case("desc") {
                        (parts[0].to_string(), OrderDirection::Desc)
                    } else {
                        (parts[0].to_string(), OrderDirection::Asc)
                    }
                })
                .collect()
        })
    }
    
    /// Parse $expand into a list of navigation properties
    pub fn parse_expand(&self) -> Option<Vec<String>> {
        self.expand.as_ref().map(|s| {
            s.split(',')
                .map(|prop| prop.trim().to_string())
                .collect()
        })
    }
    
    /// Parse simple $filter expressions (basic implementation)
    pub fn parse_filter(&self) -> Option<FilterExpression> {
        self.filter.as_ref().and_then(|f| FilterExpression::parse(f))
    }
    
    /// Validate query options
    pub fn validate(&self) -> Result<(), String> {
        if let Some(top) = self.top {
            if top < 0 || top > 1000 {
                return Err("$top must be between 0 and 1000".to_string());
            }
        }
        
        if let Some(skip) = self.skip {
            if skip < 0 {
                return Err("$skip must be non-negative".to_string());
            }
        }
        
        Ok(())
    }
}

/// Order direction for $orderby
#[derive(Debug, Clone, PartialEq)]
pub enum OrderDirection {
    Asc,
    Desc,
}

/// Basic filter expression support (simplified implementation)
#[derive(Debug, Clone)]
pub struct FilterExpression {
    pub property: String,
    pub operator: FilterOperator,
    pub value: FilterValue,
}

#[derive(Debug, Clone)]
pub enum FilterOperator {
    Eq,  // equal
    Ne,  // not equal
    Lt,  // less than
    Le,  // less than or equal
    Gt,  // greater than
    Ge,  // greater than or equal
    Contains,
    StartsWith,
    EndsWith,
}

#[derive(Debug, Clone)]
pub enum FilterValue {
    String(String),
    Number(f64),
    Integer(i64),
    Boolean(bool),
    Null,
}

impl FilterExpression {
    /// Parse a simple filter expression
    /// Examples: "name eq 'John'", "age gt 18", "active eq true"
    pub fn parse(filter: &str) -> Option<Self> {
        let parts: Vec<&str> = filter.trim().split_whitespace().collect();
        if parts.len() < 3 {
            return None;
        }
        
        let property = parts[0].to_string();
        let operator = match parts[1].to_lowercase().as_str() {
            "eq" => FilterOperator::Eq,
            "ne" => FilterOperator::Ne,
            "lt" => FilterOperator::Lt,
            "le" => FilterOperator::Le,
            "gt" => FilterOperator::Gt,
            "ge" => FilterOperator::Ge,
            _ => return None,
        };
        
        let value_str = parts[2..].join(" ");
        let value = if value_str.starts_with('\'') && value_str.ends_with('\'') {
            FilterValue::String(value_str[1..value_str.len()-1].to_string())
        } else if let Ok(num) = value_str.parse::<i64>() {
            FilterValue::Integer(num)
        } else if let Ok(num) = value_str.parse::<f64>() {
            FilterValue::Number(num)
        } else if let Ok(bool_val) = value_str.parse::<bool>() {
            FilterValue::Boolean(bool_val)
        } else if value_str.eq_ignore_ascii_case("null") {
            FilterValue::Null
        } else {
            FilterValue::String(value_str)
        };
        
        Some(FilterExpression {
            property,
            operator,
            value,
        })
    }
}

/// OData response wrapper that includes metadata
#[derive(Serialize, Debug)]
pub struct ODataCollectionResponse<T> {
    #[serde(rename = "@odata.context")]
    pub context: String,
    
    #[serde(rename = "@odata.count", skip_serializing_if = "Option::is_none")]
    pub count: Option<i64>,
    
    #[serde(rename = "@odata.nextLink", skip_serializing_if = "Option::is_none")]
    pub next_link: Option<String>,
    
    pub value: Vec<T>,
}

/// OData response for single entities
#[derive(Serialize, Debug)]
pub struct ODataEntityResponse<T> {
    #[serde(rename = "@odata.context")]
    pub context: String,
    
    #[serde(rename = "@odata.id")]
    pub id: String,
    
    #[serde(flatten)]
    pub entity: T,
}

impl<T> ODataCollectionResponse<T> {
    pub fn new(context: String, value: Vec<T>) -> Self {
        Self {
            context,
            count: None,
            next_link: None,
            value,
        }
    }
    
    pub fn with_count(mut self, count: i64) -> Self {
        self.count = Some(count);
        self
    }
    
    pub fn with_next_link(mut self, next_link: String) -> Self {
        self.next_link = Some(next_link);
        self
    }
}

impl<T> ODataEntityResponse<T> {
    pub fn new(context: String, id: String, entity: T) -> Self {
        Self {
            context,
            id,
            entity,
        }
    }
}

/// Helper function to build context URL
pub fn build_context_url(base_url: &str, entity_set: &str, select: Option<&[String]>) -> String {
    let mut context = format!("{base_url}/$metadata#{entity_set}");
    
    if let Some(props) = select {
        if !props.is_empty() && !props.contains(&"*".to_string()) {
            context.push_str(&format!("({})", props.join(",")));
        }
    }
    
    context
}

/// Helper function to apply $select to any serializable object
/// Returns a filtered HashMap containing only selected properties
pub fn apply_select<T: Serialize>(entity: &T, select: Option<&[String]>) -> Result<serde_json::Value, serde_json::Error> {
    if let Some(properties) = select {
        if properties.contains(&"*".to_string()) {
            return serde_json::to_value(entity);
        }
        
        let full_value = serde_json::to_value(entity)?;
        if let serde_json::Value::Object(full_map) = full_value {
            let mut filtered_map = serde_json::Map::new();
            for prop in properties {
                if let Some(value) = full_map.get(prop) {
                    filtered_map.insert(prop.clone(), value.clone());
                }
            }
            Ok(serde_json::Value::Object(filtered_map))
        } else {
            serde_json::to_value(entity)
        }
    } else {
        serde_json::to_value(entity)
    }
}