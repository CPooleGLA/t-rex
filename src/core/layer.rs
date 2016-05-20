//
// Copyright (c) Pirmin Kalberer. All rights reserved.
// Licensed under the MIT License. See LICENSE file in the project root for full license information.
//

#[derive(Default)]
pub struct Layer {
    pub name: String,
    pub table_name: Option<String>,
    pub geometry_field: Option<String>,
    pub geometry_type: Option<String>,
    pub fid_field: Option<String>,
    pub query_limit: Option<u32>,
    pub query: Option<String>,
}

impl Layer {
    pub fn new(name: &str) -> Layer {
        Layer { name: String::from(name), ..Default::default() }
    }
}