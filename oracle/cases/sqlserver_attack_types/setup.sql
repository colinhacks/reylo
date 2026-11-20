create table tx (
  id int not null primary key,
  l varchar(10) collate Latin1_General_100_CI_AS not null check (l in (N'éééééééééé', N'a')),
  u varchar(10) collate Latin1_General_100_CI_AS_SC_UTF8 not null check (u in (N'ȺȺȺȺȺ', N'ɐɐɐɐɐ', N'ⱥⱥⱥ', N'a')),
  n nvarchar(5) collate Latin1_General_100_CI_AS_SC not null,
  c char(4) collate Latin1_General_100_CI_AS_SC_UTF8 not null check (c in (N'éé', N'a')),
  k varchar(10) not null,
  n2 nvarchar(6) collate Latin1_General_100_CI_AS_SC not null check (n2 in (N'😀😀😀', N'a'))
);
create table nums (
  id int not null primary key,
  d decimal(38, 10) not null,
  e decimal(5, 2) not null,
  mo money not null,
  sm smallmoney not null,
  r real not null,
  fl float not null,
  ti tinyint not null
);
create table times (
  id int not null primary key,
  d2 datetime2(7) not null,
  d0 datetime2(0) not null,
  dt datetime not null,
  sdt smalldatetime not null,
  o datetimeoffset(7) not null,
  o3 datetimeoffset(3) not null,
  t time(7) not null,
  dd date not null
);
create table fx (
  id int not null primary key,
  c char(4) not null,
  nc nchar(4) not null,
  b binary(4) not null,
  cn char(4) null,
  bn binary(4) null
);
