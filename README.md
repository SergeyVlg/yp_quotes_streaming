# yp_quotes_streaming

Учебный Rust-проект: сервер генерирует котировки и стримит их клиентам по UDP, а управление подключением идет по TCP.

## Содержание

- `crates/server` — сервер котировок, принимает команду стриминга, отправляет данные и проверяет ping клиента.
- `crates/client` — клиент, подключается к серверу, подписывается на тикеры и получает котировки по UDP.
- `crates/shared` — общие структуры и протокол (`StreamCommand`, `AckResponse`, `StockQuote`, `PING`).

Проект собран как единый Cargo workspace.

## Быстрый запуск

```powershell
Set-Location "C:\Repositories\yandex_course\yp_quotes_streaming"
cargo run -p server
```

В отдельном терминале:

```powershell
Set-Location "C:\Repositories\yandex_course\yp_quotes_streaming"
$env:RUST_LOG="info"
cargo run -p client -- --server-addr 127.0.0.1:7878 --udp-port 6000 --tickers-file .\crates\client\tickers.txt
```

## Проверка

```powershell
Set-Location "C:\Repositories\yandex_course\yp_quotes_streaming"
cargo check --workspace
cargo test -p shared
```
