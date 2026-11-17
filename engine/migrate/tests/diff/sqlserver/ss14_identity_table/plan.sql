create table events (
  id bigint identity(1, 1) not null,
  kind varchar(20) not null,
  at datetime2(7) not null default (sysutcdatetime()),
  primary key clustered (id)
);
