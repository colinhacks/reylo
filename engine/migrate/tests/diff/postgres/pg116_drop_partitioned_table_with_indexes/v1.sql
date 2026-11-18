create table kept (id integer primary key);
create table request_logs (id integer not null, created_on date not null, status text) partition by range (created_on);
create index idx_request_logs_status on request_logs (status);
create index idx_request_logs_created_on on request_logs (created_on);
create table request_logs_2024 partition of request_logs for values from ('2024-01-01') to ('2025-01-01');
create table request_logs_default partition of request_logs default;
