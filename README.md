# 115 许愿助手

## 介绍
本项目实现了 115 许愿助手，通过配置 cookie 信息，可以实现自动许愿。
目前支持一个助愿账号，对应多个许愿账号。

## 系统要求

- Linux x86_64

## 运行时的目录结构

```
wish_115/app
├── wish_115 # 编译好的可执行文件
├── config.yaml # 配置文件
├── logs # 日志文件目录（程序运行会自动创建，也可以提前手动创建）
```

## 使用方法

1. 创建工作目录
   ```bash
   mkdir wish_115
   ```
2. 编译执行文件
    ```bash
   # 编译
    cargo build --release --target x86_64-unknown-linux-gnu
   # 将编译好的文件移至 app 目录
    cp target/x86_64-unknown-linux-gnu/release/wish_115 app/
    ```
3. 将根目录的 config.yaml 移至 app 目录
   ```bash 
    cp config.yaml app/
   ```
4. 修改 app/config.yaml 文件，根据注释填入你的 cookie 信息
5. 运行程序
   ```bash
   # 进入 app 目录
   cd app
   # 给程序添加执行权限
   chmod +x wish_115
   # 运行程序
   ./wish_115
   ```