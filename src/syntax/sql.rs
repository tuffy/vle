// Copyright 2026 Brian Langenberger
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::highlighter;
use crate::syntax::color;
use logos::Logos;
use ratatui::style::Color;

#[derive(Logos, Debug)]
#[logos(skip r"[ \t\n]+")]
enum SqlToken {
    #[token("ADD")]
    #[token("CONSTRAINT")]
    #[token("ALL")]
    #[token("ALTER")]
    #[token("COLUMN")]
    #[token("TABLE")]
    #[token("AND")]
    #[token("ANY")]
    #[token("AS")]
    #[token("ASC")]
    #[token("BACKUP")]
    #[token("DATABASE")]
    #[token("BEGIN")]
    #[token("BETWEEN")]
    #[token("CHECK")]
    #[token("CREATE")]
    #[token("INDEX")]
    #[token("REPLACE")]
    #[token("VIEW")]
    #[token("PROCEDURE")]
    #[token("UNIQUE")]
    #[token("DEFAULT")]
    #[token("DELETE")]
    #[token("INSERT")]
    #[token("BEFORE")]
    #[token("TRIGGER")]
    #[token("DESC")]
    #[token("DISTINCT")]
    #[token("DROP")]
    #[token("EXEC")]
    #[token("FOREIGN")]
    #[token("KEY")]
    #[token("FROM")]
    #[token("FULL")]
    #[token("OUTER")]
    #[token("EXISTS")]
    #[token("INNER")]
    #[token("JOIN")]
    #[token("GROUP")]
    #[token("BY")]
    #[token("HAVING")]
    #[token("IN")]
    #[token("IS")]
    #[token("NULL")]
    #[token("NOT")]
    #[token("LIKE")]
    #[token("LIMIT")]
    #[token("OR")]
    #[token("ORDER")]
    #[token("PRIMARY")]
    #[token("RIGHT")]
    #[token("ROWNUM")]
    #[token("SELECT")]
    #[token("INTO")]
    #[token("TOP")]
    #[token("SET")]
    #[token("TRUNCATE")]
    #[token("UNION")]
    #[token("UPDATE")]
    #[token("VALUES")]
    #[token("WHERE")]
    #[token("ON")]
    #[token("CASCADE")]
    #[token("REFERENCES")]
    #[token("ENGINE")]
    #[token("MIN")]
    #[token("MAX")]
    #[token("CONCAT")]
    #[token("COUNT")]
    Keyword,
    #[regex(r"BIT\([0-9]+\)")]
    #[regex(r"TINYINT\([0-9]+\)")]
    #[token("BOOL")]
    #[regex(r"SMALLINT\([0-9]+\)")]
    #[regex(r"MEDIUMINT\([0-9]+\)")]
    #[regex(r"INT\([0-9]+\)")]
    #[regex(r"INTEGER\([0-9]+\)")]
    #[regex(r"BIGINT\([0-9]+\)")]
    #[regex(r"FLOAT\([0-9]+\)")]
    #[regex(r"DOUBLE\([0-9]+\)")]
    Type,
    #[token("CASE")]
    #[token("WHEN")]
    #[token("IF")]
    #[token("THEN")]
    #[token("ELSE")]
    #[token("ELSEIF")]
    #[token("LOOP")]
    #[token("CONTINUE")]
    #[token("EXIT")]
    #[token("FOR")]
    #[token("FOREACH")]
    #[token("WHILE")]
    #[token("END")]
    #[token("RAISE")]
    #[token("EXCEPTION")]
    #[token("NOTICE")]
    #[token("RETURN")]
    Flow,
    #[regex("'([^']|\\')*'")]
    #[regex(r#"\"([^\\\"]|\\.)*\""#)]
    String,
}

impl TryFrom<SqlToken> for Color {
    type Error = ();

    fn try_from(t: SqlToken) -> Result<Color, ()> {
        match t {
            SqlToken::Keyword => Ok(color::KEYWORD),
            SqlToken::Type => Ok(color::TYPE),
            SqlToken::Flow => Ok(color::FLOW),
            SqlToken::String => Ok(color::STRING),
        }
    }
}

#[derive(Debug)]
pub struct Sql;

impl std::fmt::Display for Sql {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        "SQL".fmt(f)
    }
}

highlighter!(Sql, SqlToken);
