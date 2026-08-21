//! SQL DDL (Data Definition Language) Operations
//!
//! This module contains DDL operations like CREATE/DROP TABLE, DATABASE, INDEX, etc.

use alloc::vec::Vec;
use crate::sql::{QueryExecutionError, ResultSet, SqlQuery};
use crate::RemDb;

/// 执行DROP TABLE查询
pub fn execute_drop_table_query(db: &mut RemDb, query: &SqlQuery) -> Result<ResultSet, QueryExecutionError> {
    // 提取IF EXISTS和DEFERRED选项
    let mut if_exists = false;
    let mut is_deferred = false;

    if let Some((if_exists_str, is_deferred_str, _, _, _, _, _)) = query.table_def.first() {
        if_exists = if_exists_str == "true";
        is_deferred = is_deferred_str == "true";
    }

    // 调用RemDb的drop_table方法
    db.drop_table(&query.table_name, if_exists, is_deferred)
        .map_err(|err| match err {
            crate::RemDbError::NotAllowed => QueryExecutionError::NotAllowed,
            crate::RemDbError::TableNotFound => QueryExecutionError::TableNotFound,
            _ => QueryExecutionError::InternalError,
        })?;

    // 返回空结果集
    Ok(ResultSet::new(Vec::new()))
}

/// 执行CREATE DATABASE查询
pub fn execute_create_database_query(db: &mut RemDb, query: &SqlQuery) -> Result<ResultSet, QueryExecutionError> {
    // 提取数据库名称
    let database_name = query.table_name.clone();

    // 调用RemDb的create_database方法
    db.create_database(&database_name)
        .map_err(|_| QueryExecutionError::InternalError)?;

    // 返回空结果集
    Ok(ResultSet::new(Vec::new()))
}

/// 执行USE DATABASE查询
pub fn execute_use_database_query(db: &mut RemDb, query: &SqlQuery) -> Result<ResultSet, QueryExecutionError> {
    // 提取数据库名称
    let database_name = query.table_name.clone();

    // 调用RemDb的use_database方法
    db.use_database(&database_name)
        .map_err(|_| QueryExecutionError::InternalError)?;

    // 返回空结果集
    Ok(ResultSet::new(Vec::new()))
}

/// 执行CLOSE DATABASE查询
pub fn execute_close_database_query(db: &mut RemDb, query: &SqlQuery) -> Result<ResultSet, QueryExecutionError> {
    // 提取数据库名称
    let database_name = query.table_name.clone();

    // 调用RemDb的close_database方法
    db.close_database(&database_name)
        .map_err(|_| QueryExecutionError::InternalError)?;

    // 返回空结果集
    Ok(ResultSet::new(Vec::new()))
}

/// 执行DROP DATABASE查询
pub fn execute_drop_database_query(db: &mut RemDb, query: &SqlQuery) -> Result<ResultSet, QueryExecutionError> {
    // 提取数据库名称
    let database_name = query.table_name.clone();

    // 调用RemDb的drop_database方法
    db.drop_database(&database_name)
        .map_err(|_| QueryExecutionError::InternalError)?;

    // 返回空结果集
    Ok(ResultSet::new(Vec::new()))
}