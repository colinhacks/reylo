create table logs (id bigint not null, created_at date not null) partition by range (created_at);
create table logs_2024 partition of logs for values from ('2024-01-01') to ('2025-01-01');
alter table logs owner to diff_owner;
alter table logs_2024 owner to diff_owner;
