create table everything (
  id int not null primary key,
  b bit null, ti tinyint null, si smallint null, i int null, bi bigint null,
  d decimal(18, 4) null, n numeric(5, 0) null, m money null, sm smallmoney null, f float null, r real null,
  c char(4) null, vc varchar(20) null, vm varchar(max) null, nc nchar(3) null, nv nvarchar(10) null, nm nvarchar(max) null,
  bn binary(4) null, vb varbinary(16) null, vbm varbinary(max) null,
  u uniqueidentifier null, dt date null, t0 time(0) null, t7 time null, d2 datetime2(3) null, d27 datetime2 null,
  dto datetimeoffset(7) null, dtm datetime null, sdt smalldatetime null, x xml null
);
