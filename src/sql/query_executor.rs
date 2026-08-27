//! SQL查询执行器
//!
//! 该模块负责执行SQL查询并返回结果集。

use alloc::string::String;
use alloc::string::ToString;
use alloc::vec::Vec;
use std::time::Instant;

#[cfg(feature = "log")]
use crate::log::{debug, error, info};
use crate::model::model_manager::get_global_model_manager;
use crate::sql::operations::comparison::{
    compare_values, evaluate_condition_with_alias, get_field_value, extract_index_operation,
    IndexOperation,
};
use crate::sql::operations::ddl;
use crate::sql::operations::dml;
use crate::sql::operations::expression::{
    evaluate_expression, evaluate_expression_for_aggregate, evaluate_expression_without_table,
    execute_function_call,
};
use crate::sql::operations::timeseries::execute_select_timeseries_query;
use crate::sql::query_parser::{Expression, GroupByClause, JoinType};
use crate::sql::utils::{estimate_memory_usage_for_records, sort_rows_with_alias};
use crate::sql::{
    check_memory_limit, ComparisonCondition, ComparisonOperator, Condition, QueryExecutionError,
    ResultSet, SqlQuery,
};
use crate::types::{DataType, JsonStorage, TypedValue};
use crate::{MemoryTable, RemDb, RemDbError, Value, MAX_STRING_LEN};

/// 执行SQL查询
pub fn execute_query(db: &mut RemDb, query: &SqlQuery) -> Result<ResultSet, QueryExecutionError> {
    #[cfg(feature = "log")]
    debug!(
        "execute_query called: query_type={:?}, table_name={}",
        query.query_type, query.table_name
    );
    // 检查是否是时序表查询
    let is_timeseries_table = db.time_series_tables.iter().any(|table_opt| {
        if let Some(table) = table_opt {
            table.def.base.name == query.table_name
        } else {
            false
        }
    });

    // 权限检查
    match query.query_type {
        crate::sql::QueryType::Select => {
            // 检查SELECT权限
            if let Ok(has_permission) = db.check_permission(
                "root",
                &crate::rbac::Permission::Select,
                &Some(query.table_name.clone()),
                &None,
            ) {
                if !has_permission {
                    return Err(QueryExecutionError::InternalError);
                }
            } else {
                return Err(QueryExecutionError::InternalError);
            }
        }
        crate::sql::QueryType::Insert => {
            // 检查INSERT权限
            if let Ok(has_permission) = db.check_permission(
                "root",
                &crate::rbac::Permission::Insert,
                &Some(query.table_name.clone()),
                &None,
            ) {
                if !has_permission {
                    return Err(QueryExecutionError::InternalError);
                }
            } else {
                return Err(QueryExecutionError::InternalError);
            }
        }
        crate::sql::QueryType::Update => {
            // 检查UPDATE权限
            if let Ok(has_permission) = db.check_permission(
                "root",
                &crate::rbac::Permission::Update,
                &Some(query.table_name.clone()),
                &None,
            ) {
                if !has_permission {
                    return Err(QueryExecutionError::InternalError);
                }
            } else {
                return Err(QueryExecutionError::InternalError);
            }
        }
        crate::sql::QueryType::Delete => {
            // 检查DELETE权限
            if let Ok(has_permission) = db.check_permission(
                "root",
                &crate::rbac::Permission::Delete,
                &Some(query.table_name.clone()),
                &None,
            ) {
                if !has_permission {
                    return Err(QueryExecutionError::InternalError);
                }
            } else {
                return Err(QueryExecutionError::InternalError);
            }
        }
        _ => {}
    }

    match query.query_type {
        crate::sql::QueryType::Select => {
            if is_timeseries_table {
                execute_select_timeseries_query(db, query)
            } else {
                execute_select_query(db, query)
            }
        }
        crate::sql::QueryType::Insert => dml::execute_insert_query(db, query),
        crate::sql::QueryType::Update => dml::execute_update_query(db, query),
        crate::sql::QueryType::Delete => dml::execute_delete_query(db, query),
        crate::sql::QueryType::Describe => ddl::execute_describe_query(db, query),
        crate::sql::QueryType::CreateTable => ddl::execute_create_table_query(db, query),
        crate::sql::QueryType::CreateTimeSeriesTable => {
            ddl::execute_create_time_series_table_query(db, query)
        }
        crate::sql::QueryType::CreateIndex => ddl::execute_create_index_query(db, query),
        crate::sql::QueryType::ShowIndexBuildStatus => {
            ddl::execute_show_index_build_status_query(db, query)
        }
        crate::sql::QueryType::Reindex => ddl::execute_reindex_query(db, query),
        crate::sql::QueryType::ShowTables => ddl::execute_show_tables_query(db),
        crate::sql::QueryType::CreateCheckpoint => ddl::execute_create_checkpoint_query(db),
        crate::sql::QueryType::AlterTable => ddl::execute_alter_table_query(db, query),
        crate::sql::QueryType::DropTable => ddl::execute_drop_table_query(db, query),
        crate::sql::QueryType::BeginTransaction => {
            // 开始事务
            unsafe {
                crate::transaction::begin_transaction();
            }
            Ok(ResultSet::new(Vec::new()))
        }
        crate::sql::QueryType::Commit => {
            // 提交事务
            unsafe {
                crate::transaction::commit_transaction();
            }
            Ok(ResultSet::new(Vec::new()))
        }
        crate::sql::QueryType::Rollback => {
            // 回滚事务
            unsafe {
                crate::transaction::rollback_transaction();
            }
            Ok(ResultSet::new(Vec::new()))
        }
        crate::sql::QueryType::CreateDatabase => ddl::execute_create_database_query(db, query),
        crate::sql::QueryType::UseDatabase => ddl::execute_use_database_query(db, query),
        crate::sql::QueryType::CloseDatabase => ddl::execute_close_database_query(db, query),
        crate::sql::QueryType::DropDatabase => ddl::execute_drop_database_query(db, query),
        crate::sql::QueryType::CreateModel => {
            // Register the model using the global model manager
            match get_global_model_manager() {
                Ok(mut model_manager) => {
                    match model_manager.register_model(
                        query.table_name.clone(),
                        query.model_path.clone(),
                        query.model_inputs.clone(),
                        query.model_output.clone(),
                    ) {
                        Ok(_) => Ok(ResultSet::new(Vec::new())),
                        Err(e) => {
                            #[cfg(feature = "log")]
                            error!("Model registration failed: {:?}", e);
                            Err(QueryExecutionError::InternalError)
                        }
                    }
                }
                Err(_) => Err(QueryExecutionError::InternalError),
            }
        }
        crate::sql::QueryType::CreateRole => {
            // Extract role name from table_name field
            let role_name = query.table_name.clone();
            db.create_role(&role_name)
                .map_err(|_| QueryExecutionError::InternalError)?;
            Ok(ResultSet::new(Vec::new()))
        }
        crate::sql::QueryType::DropRole => {
            // Extract role name from table_name field
            let role_name = query.table_name.clone();
            db.drop_role(&role_name)
                .map_err(|_| QueryExecutionError::InternalError)?;
            Ok(ResultSet::new(Vec::new()))
        }
        crate::sql::QueryType::GrantPermission => {
            // Extract role name and permission from query fields
            let role_name = query.table_name.clone();
            // Extract permission from the first field in table_def
            if let Some((permission_str, _, _, _, _, _, _)) = query.table_def.first() {
                let permission = crate::rbac::Permission::from_str(permission_str)
                    .ok_or(QueryExecutionError::InternalError)?;
                // Extract table name from the second field
                let table_name =
                    if let Some((_, table_name, _, _, _, _, _)) = query.table_def.first() {
                        table_name.clone()
                    } else {
                        String::new()
                    };
                db.grant_permission(&role_name, permission, Some(table_name), None)
                    .map_err(|_| QueryExecutionError::InternalError)?;
                Ok(ResultSet::new(Vec::new()))
            } else {
                Err(QueryExecutionError::InternalError)
            }
        }
        crate::sql::QueryType::RevokePermission => {
            // Extract role name and permission from query fields
            let role_name = query.table_name.clone();
            // Extract permission from the first field in table_def
            if let Some((permission_str, _, _, _, _, _, _)) = query.table_def.first() {
                let permission = crate::rbac::Permission::from_str(permission_str)
                    .ok_or(QueryExecutionError::InternalError)?;
                // Extract table name from the second field
                let table_name =
                    if let Some((_, table_name, _, _, _, _, _)) = query.table_def.first() {
                        table_name.clone()
                    } else {
                        String::new()
                    };
                db.revoke_permission(&role_name, &permission, &Some(table_name), &None)
                    .map_err(|_| QueryExecutionError::InternalError)?;
                Ok(ResultSet::new(Vec::new()))
            } else {
                Err(QueryExecutionError::InternalError)
            }
        }
        crate::sql::QueryType::GrantRole => {
            // Extract username and role name from query fields
            let username = query.table_name.clone();
            // Extract role name from the first field in table_def
            if let Some((role_name, _, _, _, _, _, _)) = query.table_def.first() {
                db.grant_role(&username, role_name)
                    .map_err(|_| QueryExecutionError::InternalError)?;
                Ok(ResultSet::new(Vec::new()))
            } else {
                Err(QueryExecutionError::InternalError)
            }
        }
        crate::sql::QueryType::RevokeRole => {
            // Extract username and role name from query fields
            let username = query.table_name.clone();
            // Extract role name from the first field in table_def
            if let Some((role_name, _, _, _, _, _, _)) = query.table_def.first() {
                db.revoke_role(&username, role_name)
                    .map_err(|_| QueryExecutionError::InternalError)?;
                Ok(ResultSet::new(Vec::new()))
            } else {
                Err(QueryExecutionError::InternalError)
            }
        }
        _ => Err(QueryExecutionError::InternalError),
    }
}

