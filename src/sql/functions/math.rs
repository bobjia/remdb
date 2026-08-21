//! SQL Math Functions
//!
//! This module contains math-related function implementations like ABS, SQRT, POWER, SIN, COS, LOG, EXP, ROUND, CEIL, FLOOR, MOD.

use crate::types::DataType;
use crate::types::TypedValue;
use crate::Value;
use crate::sql::QueryExecutionError;

/// 执行ABS函数
pub fn execute_abs(args: &[TypedValue]) -> Result<TypedValue, QueryExecutionError> {
    if args.is_empty() {
        return Err(QueryExecutionError::TypeMismatch);
    }

    let arg = &args[0];

    unsafe {
        match arg.value_type {
            DataType::Int8 => Ok(TypedValue {
                value_type: DataType::Int8,
                value: Value {
                    i8: arg.value.i8.abs(),
                },
            }),
            DataType::Int16 => Ok(TypedValue {
                value_type: DataType::Int16,
                value: Value {
                    i16: arg.value.i16.abs(),
                },
            }),
            DataType::Int32 => Ok(TypedValue {
                value_type: DataType::Int32,
                value: Value {
                    i32: arg.value.i32.abs(),
                },
            }),
            DataType::Int64 => Ok(TypedValue {
                value_type: DataType::Int64,
                value: Value {
                    i64: arg.value.i64.abs(),
                },
            }),
            DataType::Float32 => Ok(TypedValue {
                value_type: DataType::Float32,
                value: Value {
                    float32: arg.value.float32.abs(),
                },
            }),
            DataType::Float64 => Ok(TypedValue {
                value_type: DataType::Float64,
                value: Value {
                    float64: arg.value.float64.abs(),
                },
            }),
            _ => Err(QueryExecutionError::TypeMismatch),
        }
    }
}

/// 执行SQRT函数
pub fn execute_sqrt(args: &[TypedValue]) -> Result<TypedValue, QueryExecutionError> {
    if args.is_empty() {
        return Err(QueryExecutionError::TypeMismatch);
    }

    let arg = &args[0];

    unsafe {
        let value = match arg.value_type {
            DataType::UInt8 => arg.value.u8 as f64,
            DataType::UInt16 => arg.value.u16 as f64,
            DataType::UInt32 => arg.value.u32 as f64,
            DataType::UInt64 => arg.value.u64 as f64,
            DataType::Int8 => arg.value.i8 as f64,
            DataType::Int16 => arg.value.i16 as f64,
            DataType::Int32 => arg.value.i32 as f64,
            DataType::Int64 => arg.value.i64 as f64,
            DataType::Float32 => arg.value.float32 as f64,
            DataType::Float64 => arg.value.float64,
            _ => return Err(QueryExecutionError::TypeMismatch),
        };

        #[cfg(feature = "std")]
        let result = value.sqrt();
        #[cfg(not(feature = "std"))]
        let result = 0.0;

        Ok(TypedValue {
            value_type: DataType::Float64,
            value: Value { float64: result },
        })
    }
}

/// 执行POWER函数
pub fn execute_power(args: &[TypedValue]) -> Result<TypedValue, QueryExecutionError> {
    if args.len() < 2 {
        return Err(QueryExecutionError::TypeMismatch);
    }

    let base_arg = &args[0];
    let exponent_arg = &args[1];

    unsafe {
        let base = match base_arg.value_type {
            DataType::UInt8 => base_arg.value.u8 as f64,
            DataType::UInt16 => base_arg.value.u16 as f64,
            DataType::UInt32 => base_arg.value.u32 as f64,
            DataType::UInt64 => base_arg.value.u64 as f64,
            DataType::Int8 => base_arg.value.i8 as f64,
            DataType::Int16 => base_arg.value.i16 as f64,
            DataType::Int32 => base_arg.value.i32 as f64,
            DataType::Int64 => base_arg.value.i64 as f64,
            DataType::Float32 => base_arg.value.float32 as f64,
            DataType::Float64 => base_arg.value.float64,
            _ => return Err(QueryExecutionError::TypeMismatch),
        };

        let exponent = match exponent_arg.value_type {
            DataType::UInt8 => exponent_arg.value.u8 as f64,
            DataType::UInt16 => exponent_arg.value.u16 as f64,
            DataType::UInt32 => exponent_arg.value.u32 as f64,
            DataType::UInt64 => exponent_arg.value.u64 as f64,
            DataType::Int8 => exponent_arg.value.i8 as f64,
            DataType::Int16 => exponent_arg.value.i16 as f64,
            DataType::Int32 => exponent_arg.value.i32 as f64,
            DataType::Int64 => exponent_arg.value.i64 as f64,
            DataType::Float32 => exponent_arg.value.float32 as f64,
            DataType::Float64 => exponent_arg.value.float64,
            _ => return Err(QueryExecutionError::TypeMismatch),
        };

        #[cfg(feature = "std")]
        let result = base.powf(exponent);
        #[cfg(not(feature = "std"))]
        let result = 0.0;

        Ok(TypedValue {
            value_type: DataType::Float64,
            value: Value { float64: result },
        })
    }
}

/// 执行SIN函数
pub fn execute_sin(args: &[TypedValue]) -> Result<TypedValue, QueryExecutionError> {
    if args.is_empty() {
        return Err(QueryExecutionError::TypeMismatch);
    }

    let arg = &args[0];

    unsafe {
        let value = match arg.value_type {
            DataType::UInt8 => arg.value.u8 as f64,
            DataType::UInt16 => arg.value.u16 as f64,
            DataType::UInt32 => arg.value.u32 as f64,
            DataType::UInt64 => arg.value.u64 as f64,
            DataType::Int8 => arg.value.i8 as f64,
            DataType::Int16 => arg.value.i16 as f64,
            DataType::Int32 => arg.value.i32 as f64,
            DataType::Int64 => arg.value.i64 as f64,
            DataType::Float32 => arg.value.float32 as f64,
            DataType::Float64 => arg.value.float64,
            _ => return Err(QueryExecutionError::TypeMismatch),
        };

        #[cfg(feature = "std")]
        let result = value.sin();
        #[cfg(not(feature = "std"))]
        let result = 0.0;

        Ok(TypedValue {
            value_type: DataType::Float64,
            value: Value { float64: result },
        })
    }
}

/// 执行COS函数
pub fn execute_cos(args: &[TypedValue]) -> Result<TypedValue, QueryExecutionError> {
    if args.is_empty() {
        return Err(QueryExecutionError::TypeMismatch);
    }

    let arg = &args[0];

    unsafe {
        let value = match arg.value_type {
            DataType::UInt8 => arg.value.u8 as f64,
            DataType::UInt16 => arg.value.u16 as f64,
            DataType::UInt32 => arg.value.u32 as f64,
            DataType::UInt64 => arg.value.u64 as f64,
            DataType::Int8 => arg.value.i8 as f64,
            DataType::Int16 => arg.value.i16 as f64,
            DataType::Int32 => arg.value.i32 as f64,
            DataType::Int64 => arg.value.i64 as f64,
            DataType::Float32 => arg.value.float32 as f64,
            DataType::Float64 => arg.value.float64,
            _ => return Err(QueryExecutionError::TypeMismatch),
        };

        #[cfg(feature = "std")]
        let result = value.cos();
        #[cfg(not(feature = "std"))]
        let result = 0.0;

        Ok(TypedValue {
            value_type: DataType::Float64,
            value: Value { float64: result },
        })
    }
}

/// 执行LOG函数
pub fn execute_log(args: &[TypedValue]) -> Result<TypedValue, QueryExecutionError> {
    if args.is_empty() {
        return Err(QueryExecutionError::TypeMismatch);
    }

    let arg = &args[0];

    unsafe {
        let value = match arg.value_type {
            DataType::UInt8 => arg.value.u8 as f64,
            DataType::UInt16 => arg.value.u16 as f64,
            DataType::UInt32 => arg.value.u32 as f64,
            DataType::UInt64 => arg.value.u64 as f64,
            DataType::Int8 => arg.value.i8 as f64,
            DataType::Int16 => arg.value.i16 as f64,
            DataType::Int32 => arg.value.i32 as f64,
            DataType::Int64 => arg.value.i64 as f64,
            DataType::Float32 => arg.value.float32 as f64,
            DataType::Float64 => arg.value.float64,
            _ => return Err(QueryExecutionError::TypeMismatch),
        };

        #[cfg(feature = "std")]
        let result = value.ln(); // 自然对数
        #[cfg(not(feature = "std"))]
        let result = 0.0;

        Ok(TypedValue {
            value_type: DataType::Float64,
            value: Value { float64: result },
        })
    }
}

/// 执行EXP函数
pub fn execute_exp(args: &[TypedValue]) -> Result<TypedValue, QueryExecutionError> {
    if args.is_empty() {
        return Err(QueryExecutionError::TypeMismatch);
    }

    let arg = &args[0];

    unsafe {
        let value = match arg.value_type {
            DataType::UInt8 => arg.value.u8 as f64,
            DataType::UInt16 => arg.value.u16 as f64,
            DataType::UInt32 => arg.value.u32 as f64,
            DataType::UInt64 => arg.value.u64 as f64,
            DataType::Int8 => arg.value.i8 as f64,
            DataType::Int16 => arg.value.i16 as f64,
            DataType::Int32 => arg.value.i32 as f64,
            DataType::Int64 => arg.value.i64 as f64,
            DataType::Float32 => arg.value.float32 as f64,
            DataType::Float64 => arg.value.float64,
            _ => return Err(QueryExecutionError::TypeMismatch),
        };

        #[cfg(feature = "std")]
        let result = value.exp();
        #[cfg(not(feature = "std"))]
        let result = 0.0;

        Ok(TypedValue {
            value_type: DataType::Float64,
            value: Value { float64: result },
        })
    }
}

/// 执行ROUND函数
pub fn execute_round(args: &[TypedValue]) -> Result<TypedValue, QueryExecutionError> {
    if args.is_empty() {
        return Err(QueryExecutionError::TypeMismatch);
    }

    let arg = &args[0];
    let decimals = if args.len() > 1 {
        unsafe {
            match args[1].value_type {
                DataType::Int8 => args[1].value.i8 as i32,
                DataType::Int16 => args[1].value.i16 as i32,
                DataType::Int32 => args[1].value.i32,
                DataType::Int64 => args[1].value.i64 as i32,
                DataType::UInt8 => args[1].value.u8 as i32,
                DataType::UInt16 => args[1].value.u16 as i32,
                DataType::UInt32 => args[1].value.u32 as i32,
                DataType::UInt64 => args[1].value.u64 as i32,
                _ => 0,
            }
        }
    } else {
        0
    };

    unsafe {
        match arg.value_type {
            DataType::Float32 => {
                #[cfg(feature = "std")]
                let factor = 10.0f32.powi(decimals);
                #[cfg(feature = "std")]
                let result = (arg.value.float32 * factor).round() / factor;
                #[cfg(not(feature = "std"))]
                let result = arg.value.float32;
                Ok(TypedValue {
                    value_type: DataType::Float32,
                    value: Value { float32: result },
                })
            }
            DataType::Float64 => {
                #[cfg(feature = "std")]
                let factor = 10.0f64.powi(decimals);
                #[cfg(feature = "std")]
                let result = (arg.value.float64 * factor).round() / factor;
                #[cfg(not(feature = "std"))]
                let result = arg.value.float64;
                Ok(TypedValue {
                    value_type: DataType::Float64,
                    value: Value { float64: result },
                })
            }
            _ => {
                // 对于整数类型，直接返回原值
                Ok(arg.clone())
            }
        }
    }
}

/// 执行CEIL函数
pub fn execute_ceil(args: &[TypedValue]) -> Result<TypedValue, QueryExecutionError> {
    if args.is_empty() {
        return Err(QueryExecutionError::TypeMismatch);
    }

    let arg = &args[0];

    unsafe {
        match arg.value_type {
            DataType::Float32 => {
                let result = {
                    #[cfg(feature = "std")]
                    let val = arg.value.float32.ceil();
                    #[cfg(not(feature = "std"))]
                    let val = arg.value.float32;
                    val
                };
                Ok(TypedValue {
                    value_type: DataType::Float32,
                    value: Value { float32: result },
                })
            }
            DataType::Float64 => {
                let result = {
                    #[cfg(feature = "std")]
                    let val = arg.value.float64.ceil();
                    #[cfg(not(feature = "std"))]
                    let val = arg.value.float64;
                    val
                };
                Ok(TypedValue {
                    value_type: DataType::Float64,
                    value: Value { float64: result },
                })
            }
            _ => {
                // 对于整数类型，直接返回原值
                Ok(arg.clone())
            }
        }
    }
}

/// 执行FLOOR函数
pub fn execute_floor(args: &[TypedValue]) -> Result<TypedValue, QueryExecutionError> {
    if args.is_empty() {
        return Err(QueryExecutionError::TypeMismatch);
    }

    let arg = &args[0];

    unsafe {
        match arg.value_type {
            DataType::Float32 => {
                let result = {
                    #[cfg(feature = "std")]
                    let val = arg.value.float32.floor();
                    #[cfg(not(feature = "std"))]
                    let val = arg.value.float32;
                    val
                };
                Ok(TypedValue {
                    value_type: DataType::Float32,
                    value: Value { float32: result },
                })
            }
            DataType::Float64 => {
                let result = {
                    #[cfg(feature = "std")]
                    let val = arg.value.float64.floor();
                    #[cfg(not(feature = "std"))]
                    let val = arg.value.float64;
                    val
                };
                Ok(TypedValue {
                    value_type: DataType::Float64,
                    value: Value { float64: result },
                })
            }
            _ => {
                // 对于整数类型，直接返回原值
                Ok(arg.clone())
            }
        }
    }
}

/// 执行MOD函数
pub fn execute_mod(args: &[TypedValue]) -> Result<TypedValue, QueryExecutionError> {
    if args.len() < 2 {
        return Err(QueryExecutionError::TypeMismatch);
    }

    let dividend_arg = &args[0];
    let divisor_arg = &args[1];

    unsafe {
        match (dividend_arg.value_type, divisor_arg.value_type) {
            // 整数类型
            (DataType::UInt8, DataType::UInt8) => Ok(TypedValue {
                value_type: DataType::UInt8,
                value: Value {
                    u8: dividend_arg.value.u8 % divisor_arg.value.u8,
                },
            }),
            (DataType::UInt16, DataType::UInt16) => Ok(TypedValue {
                value_type: DataType::UInt16,
                value: Value {
                    u16: dividend_arg.value.u16 % divisor_arg.value.u16,
                },
            }),
            (DataType::UInt32, DataType::UInt32) => Ok(TypedValue {
                value_type: DataType::UInt32,
                value: Value {
                    u32: dividend_arg.value.u32 % divisor_arg.value.u32,
                },
            }),
            (DataType::UInt64, DataType::UInt64) => Ok(TypedValue {
                value_type: DataType::UInt64,
                value: Value {
                    u64: dividend_arg.value.u64 % divisor_arg.value.u64,
                },
            }),
            (DataType::Int8, DataType::Int8) => Ok(TypedValue {
                value_type: DataType::Int8,
                value: Value {
                    i8: dividend_arg.value.i8 % divisor_arg.value.i8,
                },
            }),
            (DataType::Int16, DataType::Int16) => Ok(TypedValue {
                value_type: DataType::Int16,
                value: Value {
                    i16: dividend_arg.value.i16 % divisor_arg.value.i16,
                },
            }),
            (DataType::Int32, DataType::Int32) => Ok(TypedValue {
                value_type: DataType::Int32,
                value: Value {
                    i32: dividend_arg.value.i32 % divisor_arg.value.i32,
                },
            }),
            (DataType::Int64, DataType::Int64) => Ok(TypedValue {
                value_type: DataType::Int64,
                value: Value {
                    i64: dividend_arg.value.i64 % divisor_arg.value.i64,
                },
            }),
            // 浮点数类型
            (DataType::Float32, DataType::Float32) => Ok(TypedValue {
                value_type: DataType::Float32,
                value: Value {
                    float32: dividend_arg.value.float32 % divisor_arg.value.float32,
                },
            }),
            (DataType::Float64, DataType::Float64) => Ok(TypedValue {
                value_type: DataType::Float64,
                value: Value {
                    float64: dividend_arg.value.float64 % divisor_arg.value.float64,
                },
            }),
            // 混合类型，转换为浮点数
            _ => {
                let dividend = match dividend_arg.value_type {
                    DataType::UInt8 => dividend_arg.value.u8 as f64,
                    DataType::UInt16 => dividend_arg.value.u16 as f64,
                    DataType::UInt32 => dividend_arg.value.u32 as f64,
                    DataType::UInt64 => dividend_arg.value.u64 as f64,
                    DataType::Int8 => dividend_arg.value.i8 as f64,
                    DataType::Int16 => dividend_arg.value.i16 as f64,
                    DataType::Int32 => dividend_arg.value.i32 as f64,
                    DataType::Int64 => dividend_arg.value.i64 as f64,
                    DataType::Float32 => dividend_arg.value.float32 as f64,
                    DataType::Float64 => dividend_arg.value.float64,
                    _ => return Err(QueryExecutionError::TypeMismatch),
                };

                let divisor = match divisor_arg.value_type {
                    DataType::UInt8 => divisor_arg.value.u8 as f64,
                    DataType::UInt16 => divisor_arg.value.u16 as f64,
                    DataType::UInt32 => divisor_arg.value.u32 as f64,
                    DataType::UInt64 => divisor_arg.value.u64 as f64,
                    DataType::Int8 => divisor_arg.value.i8 as f64,
                    DataType::Int16 => divisor_arg.value.i16 as f64,
                    DataType::Int32 => divisor_arg.value.i32 as f64,
                    DataType::Int64 => divisor_arg.value.i64 as f64,
                    DataType::Float32 => divisor_arg.value.float32 as f64,
                    DataType::Float64 => divisor_arg.value.float64,
                    _ => return Err(QueryExecutionError::TypeMismatch),
                };

                let result = dividend % divisor;

                Ok(TypedValue {
                    value_type: DataType::Float64,
                    value: Value { float64: result },
                })
            }
        }
    }
}