create table conv (
  id int identity(1, 1) not null primary key,
  code varchar(10) not null check (code in ('1', '01', '+1', ' 1')),
  constraint conv_code unique (code)
);
create table words (
  id int identity(1, 1) not null primary key,
  w varchar(10) collate SQL_Latin1_General_CP1_CI_AS not null check (w in (N'ss', N'ß', N'ab', N'a­b', N'ae', N'æ')),
  constraint words_w unique (w)
);
create table big (
  id int identity(1, 1) not null primary key,
  k bigint not null check (k in (9007199254740992, 9007199254740993)),
  constraint big_k unique (k)
);
create table plainpk (
  id int not null primary key,
  v int null
);
create table soft (
  id int identity(1, 1) not null primary key,
  k int not null,
  deleted bit not null
);
create unique index soft_k on soft (k) where deleted = 0;
create table dis (
  id int identity(1, 1) not null primary key,
  k int not null
);
create unique index dis_k on dis (k);
alter index dis_k on dis disable;
create table idk (
  id int not null,
  v int null
);
create unique index idk_id on idk (id) with (ignore_dup_key = on);
create table idk2 (
  id int not null default 0,
  v int null
);
create unique index idk2_id on idk2 (id) with (ignore_dup_key = on);
create table trig (
  id int not null primary key,
  v int null
);
create table tt (
  id int not null primary key,
  v int not null,
  vf datetime2 generated always as row start not null,
  vt datetime2 generated always as row end not null,
  period for system_time (vf, vt)
) with (system_versioning = on (history_table = dbo.tt_history));
GO
create trigger trig_after on trig after insert, update, delete as select count(*) as n from inserted;
GO
create view vpk as select p.id, p.v from dbo.plainpk p cross join (values (1), (2)) x (n);
GO
insert into tt (id, v) values (1, 1);
GO
update tt set v = 2 where id = 1;
GO
update tt set v = 3 where id = 1;
