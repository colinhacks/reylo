create table lines (id int not null primary key, qty bigint not null, price decimal(10, 2) not null, total as qty * price persisted, label as concat(N'#', qty));
create index lines_total on lines (total);
