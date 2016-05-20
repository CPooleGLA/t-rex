//
// Copyright (c) Pirmin Kalberer. All rights reserved.
// Licensed under the MIT License. See LICENSE file in the project root for full license information.
//

use datasource::datasource::DatasourceInput;
use postgres::{Connection, SslMode};
use postgres::rows::Row;
use core::feature::{Feature,FeatureAttr};
use core::geom::*;
use core::grid::Extent;
use core::layer::Layer;
use config::Config;
use toml;


impl GeometryType {
    fn from_geom_field(row: &Row, idx: &str, type_name: &str) -> GeometryType {
        match type_name {
            "POINT"              => GeometryType::Point(row.get::<_, Point>(idx)),
            "LINESTRING"         => GeometryType::LineString(row.get::<_, LineString>(idx)),
            "POLYGON"            => GeometryType::Polygon(row.get::<_, Polygon>(idx)),
            "MULTIPOINT"         => GeometryType::MultiPoint(row.get::<_, MultiPoint>(idx)),
            "MULTILINESTRING"    => GeometryType::MultiLineString(row.get::<_, MultiLineString>(idx)),
            "MULTIPOLYGON"       => GeometryType::MultiPolygon(row.get::<_, MultiPolygon>(idx)),
            "GEOMETRYCOLLECTION" => GeometryType::GeometryCollection(row.get::<_, GeometryCollection>(idx)),
            _                    => panic!("Unknown geometry type")
        }
    }
}

struct FeatureRow<'a> {
    layer: &'a Layer,
    row: &'a Row<'a>,
    attrs: Vec<FeatureAttr>,  // temporary
}

impl<'a> Feature for FeatureRow<'a> {
    fn fid(&self) -> Option<u64> { None } //TODO
    fn attributes(&self) -> &Vec<FeatureAttr> { &self.attrs } //TODO
    fn geometry(&self) -> GeometryType {
        GeometryType::from_geom_field(
            &self.row,
            &self.layer.geometry_field.as_ref().unwrap(),
            &self.layer.geometry_type.as_ref().unwrap()
        )
    }
}

pub struct PostgisInput {
    pub connection_url: String
}

impl PostgisInput {
    pub fn detect_layers(&self) -> Vec<Layer> {
        let mut layers: Vec<Layer> = Vec::new();
        let conn = Connection::connect(&self.connection_url as &str, SslMode::None).unwrap();
        let stmt = conn.prepare("SELECT * FROM geometry_columns").unwrap();
        for row in &stmt.query(&[]).unwrap() {
            let table_name: String = row.get("f_table_name");
            let geometry_column: String = row.get("f_geometry_column");
            let srid: i32 = row.get("srid");
            let geomtype: String = row.get("type");
            let mut layer = Layer::new(&table_name);
            layer.table_name = Some(table_name.clone());
            layer.geometry_field = Some(geometry_column.clone());
            layer.geometry_type = Some(geomtype.clone());
            layers.push(layer);
        }
        layers
    }
    fn query(&self, layer: &Layer, zoom: u16) -> String {
        let mut sql = match layer.query.as_ref() {
            Some(q) => q.clone(),
            None => format!("SELECT {} FROM {}",
                layer.geometry_field.as_ref().unwrap(),
                layer.table_name.as_ref().unwrap())
        };
        sql.push_str(&format!(" WHERE ST_Intersects({},ST_MakeEnvelope($1,$2,$3,$4,3857))",
            layer.geometry_field.as_ref().unwrap()));
        if let Some(n) = layer.query_limit {
            sql.push_str(&format!(" LIMIT {}", n));
        }
        sql
    }
}

impl DatasourceInput for PostgisInput {
    fn retrieve_features<F>(&self, layer: &Layer, extent: &Extent, zoom: u16, mut read: F)
        where F : FnMut(&Feature) {
        let conn = Connection::connect(&self.connection_url as &str, SslMode::None).unwrap();
        let stmt = conn.prepare(&self.query(&layer, zoom)).unwrap();
        for row in &stmt.query(&[&extent.minx, &extent.miny, &extent.maxx, &extent.maxy]).unwrap() {
            let feature = FeatureRow { layer: layer, row: &row, attrs: vec![] };
            read(&feature)
        }
    }
}

impl Config<PostgisInput> for PostgisInput {
    fn from_config(config: &toml::Value) -> Option<Self> {
        config.lookup("datasource.url")
            .and_then(|val| val.as_str())
            .and_then(|url| Some(PostgisInput { connection_url: url.to_string() }))
    }
}

#[cfg(test)] use std::io::{self,Write};
#[cfg(test)] use std::env;
#[cfg(test)] use postgis;

#[test]
pub fn test_from_geom_fields() {
    let conn: Connection = match env::var("DBCONN") {
        Result::Ok(val) => Connection::connect(&val as &str, SslMode::None),
        Result::Err(_) => { write!(&mut io::stdout(), "skipped ").unwrap(); return; }
    }.unwrap();
    let stmt = conn.prepare("SELECT wkb_geometry FROM ne_10m_populated_places LIMIT 1").unwrap();
    for row in &stmt.query(&[]).unwrap() {
        let geom = row.get::<_, Point>("wkb_geometry");
        assert_eq!(&*format!("{:?}", geom),
            "SRID=3857;POINT(-6438719.622820721 -4093437.7144101723)");
        let geom = GeometryType::from_geom_field(&row, "wkb_geometry", "POINT");
        assert_eq!(&*format!("{:?}", geom),
            "Point(SRID=3857;POINT(-6438719.622820721 -4093437.7144101723))");
    }

    let stmt = conn.prepare("SELECT wkb_geometry FROM rivers_lake_centerlines WHERE ST_NPoints(wkb_geometry)<10 LIMIT 1").unwrap();
    for row in &stmt.query(&[]).unwrap() {
        let geom = GeometryType::from_geom_field(&row, "wkb_geometry", "LINESTRING");
        assert_eq!(&*format!("{:?}", geom),
            "LineString(LineString { points: [SRID=3857;POINT(18672061.098933436 -5690573.725394946), SRID=3857;POINT(18671798.382036217 -5692123.11701991), SRID=3857;POINT(18671707.790002696 -5693530.713572942), SRID=3857;POINT(18671789.322832868 -5694822.281317252), SRID=3857;POINT(18672061.098933436 -5695997.770001522), SRID=3857;POINT(18670620.68560042 -5698245.837796968), SRID=3857;POINT(18668283.41113552 -5700403.997584983), SRID=3857;POINT(18666082.024720907 -5701179.511527114), SRID=3857;POINT(18665148.926775623 -5699253.775757339)] })");
    }
    /* row.get panics for multi-geometries: https://github.com/andelf/rust-postgis/issues/6
    let stmt = conn.prepare("SELECT wkb_geometry FROM ne_10m_rivers_lake_centerlines WHERE ST_NPoints(wkb_geometry)<10 LIMIT 1").unwrap();
    for row in &stmt.query(&[]).unwrap() {
        let geom = row.get::<_, postgis::MultiLineString<postgis::Point<EPSG_3857>>>("wkb_geometry");
        assert_eq!(&*format!("{:#?}", geom),
            "SRID=3857;MULTILINESTRING((5959308.21223679 7539958.36540974,5969998.07219252 7539958.36540974,5972498.41231776 7539118.00291568,5977308.84929784 7535385.96203562))");
    }*/
}

#[test]
pub fn test_detect_layers() {
    let pg: PostgisInput = match env::var("DBCONN") {
        Result::Ok(val) => Some(PostgisInput {connection_url: val}),
        Result::Err(_) => { write!(&mut io::stdout(), "skipped ").unwrap(); return; }
    }.unwrap();
    let layers = pg.detect_layers();
    assert_eq!(layers[0].name, "ne_10m_populated_places");
}

#[test]
pub fn test_feature_query() {
    let pg = PostgisInput {connection_url: "postgresql://pi@localhost/osm2vectortiles".to_string()};
    let mut layer = Layer::new("points");
    layer.table_name = Some(String::from("osm_place_point"));
    layer.geometry_field = Some(String::from("geometry"));
    assert_eq!(pg.query(&layer, 10),
        "SELECT geometry FROM osm_place_point WHERE ST_Intersects(geometry,ST_MakeEnvelope($1,$2,$3,$4,3857))");

    layer.query_limit = Some(1);
    assert_eq!(pg.query(&layer, 10),
        "SELECT geometry FROM osm_place_point WHERE ST_Intersects(geometry,ST_MakeEnvelope($1,$2,$3,$4,3857)) LIMIT 1");

    layer.query = Some(String::from("SELECT geometry AS geom FROM osm_place_point"));
    assert_eq!(pg.query(&layer, 10),
        "SELECT geometry AS geom FROM osm_place_point WHERE ST_Intersects(geometry,ST_MakeEnvelope($1,$2,$3,$4,3857)) LIMIT 1");
}

#[test]
pub fn test_retrieve_features() {
    let pg: PostgisInput = match env::var("DBCONN") {
        Result::Ok(val) => Some(PostgisInput {connection_url: val}),
        Result::Err(_) => { write!(&mut io::stdout(), "skipped ").unwrap(); return; }
    }.unwrap();
    let mut layer = Layer::new("points");
    layer.table_name = Some(String::from("ne_10m_populated_places"));
    layer.geometry_field = Some(String::from("wkb_geometry"));
    layer.geometry_type = Some(String::from("POINT"));
    layer.query_limit = Some(1);
    let extent = Extent {minx: 958826.08, miny: 5987771.04, maxx: 978393.96, maxy: 6007338.92};
    pg.retrieve_features(&layer, &extent, 10, |feat| {
        assert_eq!("Point(\n    SRID=3857;POINT(960328.5530940875 6000593.929181342)\n)", &*format!("{:#?}", feat.geometry()));
        assert_eq!(0, feat.attributes().len());
        assert_eq!(None, feat.fid());
    });
}