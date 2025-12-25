-- 测试DDL文件
CREATE TABLE product (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    price REAL NOT NULL,
    stock INTEGER,
    category TEXT
);

CREATE TABLE order (
    id INTEGER PRIMARY KEY,
    user_id INTEGER NOT NULL,
    total_price REAL NOT NULL,
    created_at TIMESTAMP NOT NULL,
    status TEXT NOT NULL
);
