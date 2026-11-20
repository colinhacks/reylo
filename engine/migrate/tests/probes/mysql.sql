-- One probe per property MySQL 8.4's DDL can set: two schemas that differ in that property alone,
-- each planned both ways by tests/second_opinion.rs. Accounts 'gap_u1'@'%' and 'gap_u2'@'localhost'
-- and the role 'gap_role' exist.

-- ===== grants (known: refused as a statement past the shadow; the harness must flag them)
-- probe: grant-table
-- base:
create table t (id int primary key, a int);
-- v2:
grant select on t to 'gap_u1'@'%';

-- probe: grant-database
-- base:
create table t (id int primary key, a int);
-- v2:
grant select on * to 'gap_u1'@'%';

-- probe: grant-column
-- base:
create table t (id int primary key, a int);
-- v2:
grant select (a) on t to 'gap_u1'@'%';

-- probe: grant-routine
-- base:
create procedure p() select 1;
-- v2:
grant execute on procedure p to 'gap_u1'@'%';

-- probe: grant-option
-- base:
create table t (id int primary key, a int);
-- v1:
grant select on t to 'gap_u1'@'%';
-- v2:
grant select on t to 'gap_u1'@'%' with grant option;

-- probe: grant-role
-- base:
create table t (id int primary key, a int);
-- v2:
grant insert on t to 'gap_role';

-- ===== table options
-- probe: table-engine
-- v1:
create table t (id int primary key) engine = InnoDB;
-- v2:
create table t (id int primary key) engine = MyISAM;

-- probe: table-engine-memory
-- v1:
create table t (id int primary key) engine = InnoDB;
-- v2:
create table t (id int primary key) engine = MEMORY;

-- probe: table-row-format
-- v1:
create table t (id int primary key);
-- v2:
create table t (id int primary key) row_format = COMPACT;

-- probe: table-row-format-compressed
-- v1:
create table t (id int primary key);
-- v2:
create table t (id int primary key) row_format = COMPRESSED key_block_size = 8;

-- probe: table-key-block-size
-- v1:
create table t (id int primary key) row_format = COMPRESSED key_block_size = 8;
-- v2:
create table t (id int primary key) row_format = COMPRESSED key_block_size = 4;

-- probe: table-stats-persistent
-- v1:
create table t (id int primary key);
-- v2:
create table t (id int primary key) stats_persistent = 0;

-- probe: table-stats-auto-recalc
-- v1:
create table t (id int primary key);
-- v2:
create table t (id int primary key) stats_auto_recalc = 0;

-- probe: table-stats-sample-pages
-- v1:
create table t (id int primary key);
-- v2:
create table t (id int primary key) stats_sample_pages = 50;

-- probe: table-comment
-- v1:
create table t (id int primary key);
-- v2:
create table t (id int primary key) comment = 'things';

-- probe: table-charset
-- v1:
create table t (id int primary key, a varchar(10)) default charset = utf8mb4;
-- v2:
create table t (id int primary key, a varchar(10)) default charset = latin1;

-- probe: table-collate-only
-- v1:
create table t (id int primary key) default collate = utf8mb4_0900_ai_ci;
-- v2:
create table t (id int primary key) default collate = utf8mb4_bin;

-- probe: table-collate-columns-kept
-- v1:
create table t (id int primary key, a varchar(10) collate utf8mb4_0900_ai_ci) default collate = utf8mb4_0900_ai_ci;
-- v2:
create table t (id int primary key, a varchar(10) collate utf8mb4_0900_ai_ci) default collate = utf8mb4_bin;

-- probe: table-autoextend-size
-- v1:
create table t (id int primary key);
-- v2:
create table t (id int primary key) autoextend_size = 4M;

-- probe: table-max-rows
-- v1:
create table t (id int primary key);
-- v2:
create table t (id int primary key) max_rows = 1000;

-- probe: table-min-rows
-- v1:
create table t (id int primary key);
-- v2:
create table t (id int primary key) min_rows = 10;

-- probe: table-avg-row-length
-- v1:
create table t (id int primary key);
-- v2:
create table t (id int primary key) avg_row_length = 100;

-- probe: table-pack-keys
-- v1:
create table t (id int primary key);
-- v2:
create table t (id int primary key) pack_keys = 1;

-- probe: table-checksum
-- v1:
create table t (id int primary key) engine = MyISAM;
-- v2:
create table t (id int primary key) engine = MyISAM checksum = 1;

-- probe: table-delay-key-write
-- v1:
create table t (id int primary key) engine = MyISAM;
-- v2:
create table t (id int primary key) engine = MyISAM delay_key_write = 1;

-- probe: table-compression
-- v1:
create table t (id int primary key);
-- v2:
create table t (id int primary key) compression = 'zlib';

-- probe: table-tablespace
-- v1:
create table t (id int primary key);
-- v2:
create table t (id int primary key) tablespace = innodb_system;

-- probe: table-engine-attribute
-- v1:
create table t (id int primary key);
-- v2:
create table t (id int primary key) engine_attribute = '{"a": 1}';

-- probe: table-insert-method
-- base:
create table a1 (id int) engine = MyISAM;
create table a2 (id int) engine = MyISAM;
-- v1:
create table m (id int) engine = MERGE union = (a1, a2);
-- v2:
create table m (id int) engine = MERGE union = (a1, a2) insert_method = last;

-- probe: table-union
-- base:
create table a1 (id int) engine = MyISAM;
create table a2 (id int) engine = MyISAM;
-- v1:
create table m (id int) engine = MERGE union = (a1);
-- v2:
create table m (id int) engine = MERGE union = (a1, a2);

-- probe: table-auto-increment-start
-- base:
create table s (id int primary key);
-- v2:
create table t (id int auto_increment primary key) auto_increment = 1000;

-- probe: table-auto-increment-existing
-- base:
create table t (id int auto_increment primary key);
-- v2:
alter table t auto_increment = 1000;

-- probe: table-name-case
-- v1:
create table Users (id int primary key);
-- v2:
create table users (id int primary key);

-- probe: table-connection
-- v1:
create table t (id int primary key) engine = MyISAM;
-- v2:
create table t (id int primary key) engine = MyISAM connection = 'x';

-- probe: table-secondary-engine
-- v1:
create table t (id int primary key);
-- v2:
create table t (id int primary key) secondary_engine = rapid;

-- probe: table-disable-keys
-- base:
create table t (id int primary key, a int, key ia (a)) engine = MyISAM;
-- v2:
alter table t disable keys;

-- ===== column attributes
-- probe: column-added
-- base:
create table t (id int primary key);
-- v2:
alter table t add column a int;

-- probe: column-type
-- v1:
create table t (id int primary key, a int);
-- v2:
create table t (id int primary key, a bigint);

-- probe: column-unsigned
-- v1:
create table t (id int primary key, a int);
-- v2:
create table t (id int primary key, a int unsigned);

-- probe: column-zerofill
-- v1:
create table t (id int primary key, a int);
-- v2:
create table t (id int primary key, a int zerofill);

-- probe: column-display-width
-- v1:
create table t (id int primary key, a tinyint);
-- v2:
create table t (id int primary key, a tinyint(1));

-- probe: column-null
-- v1:
create table t (id int primary key, a int);
-- v2:
create table t (id int primary key, a int not null);

-- probe: column-default
-- v1:
create table t (id int primary key, a int);
-- v2:
create table t (id int primary key, a int default 5);

-- probe: column-default-expression
-- v1:
create table t (id int primary key, a int default 1);
-- v2:
create table t (id int primary key, a int default (1 + 1));

-- probe: column-invisible
-- v1:
create table t (id int primary key, a int);
-- v2:
create table t (id int primary key, a int invisible);

-- probe: column-comment
-- v1:
create table t (id int primary key, a int);
-- v2:
create table t (id int primary key, a int comment 'the a');

-- probe: column-collate
-- v1:
create table t (id int primary key, a varchar(10));
-- v2:
create table t (id int primary key, a varchar(10) collate utf8mb4_bin);

-- probe: column-charset
-- v1:
create table t (id int primary key, a varchar(10));
-- v2:
create table t (id int primary key, a varchar(10) character set latin1);

-- probe: column-format
-- v1:
create table t (id int primary key, a int);
-- v2:
create table t (id int primary key, a int column_format fixed);

-- probe: column-storage
-- v1:
create table t (id int primary key, a int);
-- v2:
create table t (id int primary key, a int storage memory);

-- probe: column-engine-attribute
-- v1:
create table t (id int primary key, a int);
-- v2:
create table t (id int primary key, a int engine_attribute '{"x": 1}');

-- probe: column-on-update
-- v1:
create table t (id int primary key, a timestamp null default current_timestamp);
-- v2:
create table t (id int primary key, a timestamp null default current_timestamp on update current_timestamp);

-- probe: column-fsp
-- v1:
create table t (id int primary key, a datetime);
-- v2:
create table t (id int primary key, a datetime(6));

-- probe: column-enum-values
-- v1:
create table t (id int primary key, a enum('x', 'y'));
-- v2:
create table t (id int primary key, a enum('x', 'y', 'z'));

-- probe: column-set-values
-- v1:
create table t (id int primary key, a set('x', 'y'));
-- v2:
create table t (id int primary key, a set('x', 'y', 'z'));

-- probe: column-srid
-- v1:
create table t (id int primary key, g point not null);
-- v2:
create table t (id int primary key, g point not null srid 4326);

-- probe: column-generated
-- v1:
create table t (id int primary key, a int as (id * 2));
-- v2:
create table t (id int primary key, a int as (id * 3));

-- probe: column-generated-stored
-- v1:
create table t (id int primary key, a int as (id * 2) virtual);
-- v2:
create table t (id int primary key, a int as (id * 2) stored);

-- probe: column-auto-increment
-- v1:
create table t (id int primary key);
-- v2:
create table t (id int primary key auto_increment);

-- probe: column-name-case
-- v1:
create table t (id int primary key, a int);
-- v2:
create table t (id int primary key, A int);

-- probe: column-serial
-- v1:
create table t (id bigint unsigned not null primary key);
-- v2:
create table t (id serial primary key);

-- probe: column-binary
-- v1:
create table t (id int primary key, a char(10));
-- v2:
create table t (id int primary key, a binary(10));

-- probe: column-json
-- v1:
create table t (id int primary key, a text);
-- v2:
create table t (id int primary key, a json);

-- ===== indexes
-- probe: index-added
-- base:
create table t (id int primary key, a int, b varchar(100));
-- v2:
create index ia on t (a);

-- probe: index-unique
-- base:
create table t (id int primary key, a int, b varchar(100));
-- v1:
create index ia on t (a);
-- v2:
create unique index ia on t (a);

-- probe: index-invisible
-- base:
create table t (id int primary key, a int, b varchar(100));
-- v1:
create index ia on t (a);
-- v2:
create index ia on t (a) invisible;

-- probe: index-comment
-- base:
create table t (id int primary key, a int, b varchar(100));
-- v1:
create index ia on t (a);
-- v2:
create index ia on t (a) comment 'by a';

-- probe: index-prefix
-- base:
create table t (id int primary key, a int, b varchar(100));
-- v1:
create index ib on t (b(10));
-- v2:
create index ib on t (b(20));

-- probe: index-desc
-- base:
create table t (id int primary key, a int, b varchar(100));
-- v1:
create index ia on t (a);
-- v2:
create index ia on t (a desc);

-- probe: index-expression
-- base:
create table t (id int primary key, a int, b varchar(100));
-- v1:
create index ie on t ((a + 1));
-- v2:
create index ie on t ((a + 2));

-- probe: index-column-order
-- base:
create table t (id int primary key, a int, b varchar(100));
-- v1:
create index iab on t (a, b);
-- v2:
create index iab on t (b, a);

-- probe: index-using-hash-memory
-- base:
create table t (id int primary key, a int) engine = MEMORY;
-- v1:
create index ia on t (a) using btree;
-- v2:
create index ia on t (a) using hash;

-- probe: index-key-block-size
-- base:
create table t (id int primary key, a int) engine = MyISAM;
-- v1:
create index ia on t (a);
-- v2:
create index ia on t (a) key_block_size = 2048;

-- probe: index-fulltext
-- base:
create table t (id int primary key, b varchar(100));
-- v2:
create fulltext index fb on t (b);

-- probe: index-fulltext-parser
-- base:
create table t (id int primary key, b varchar(100));
-- v1:
create fulltext index fb on t (b);
-- v2:
create fulltext index fb on t (b) with parser ngram;

-- probe: index-spatial
-- base:
create table t (id int primary key, g point not null srid 0);
-- v2:
create spatial index sg on t (g);

-- probe: index-engine-attribute
-- base:
create table t (id int primary key, a int);
-- v1:
create index ia on t (a);
-- v2:
create index ia on t (a) engine_attribute '{"x": 1}';

-- probe: primary-key-columns
-- v1:
create table t (id int, a int not null, primary key (id));
-- v2:
create table t (id int, a int not null, primary key (id, a));

-- probe: unique-constraint-name
-- v1:
create table t (id int primary key, a int, constraint u1 unique (a));
-- v2:
create table t (id int primary key, a int, constraint u2 unique (a));

-- ===== constraints
-- probe: check-added
-- base:
create table t (id int primary key, a int);
-- v2:
alter table t add constraint c check (a > 0);

-- probe: check-changed
-- v1:
create table t (id int primary key, a int, constraint c check (a > 0));
-- v2:
create table t (id int primary key, a int, constraint c check (a > 1));

-- probe: check-not-enforced
-- v1:
create table t (id int primary key, a int, constraint c check (a > 0));
-- v2:
create table t (id int primary key, a int, constraint c check (a > 0) not enforced);

-- probe: fk-added
-- base:
create table p (id int primary key);
create table c (id int primary key, p int);
-- v2:
alter table c add constraint fk foreign key (p) references p (id);

-- probe: fk-on-delete
-- base:
create table p (id int primary key);
-- v1:
create table c (id int primary key, p int, constraint fk foreign key (p) references p (id));
-- v2:
create table c (id int primary key, p int, constraint fk foreign key (p) references p (id) on delete cascade);

-- probe: fk-on-update
-- base:
create table p (id int primary key);
-- v1:
create table c (id int primary key, p int, constraint fk foreign key (p) references p (id));
-- v2:
create table c (id int primary key, p int, constraint fk foreign key (p) references p (id) on update set null);

-- probe: fk-referenced-column
-- base:
create table p (id int primary key, k int unique);
-- v1:
create table c (id int primary key, p int, constraint fk foreign key (p) references p (id));
-- v2:
create table c (id int primary key, p int, constraint fk foreign key (p) references p (k));

-- probe: fk-name
-- base:
create table p (id int primary key);
-- v1:
create table c (id int primary key, p int, constraint fk1 foreign key (p) references p (id));
-- v2:
create table c (id int primary key, p int, constraint fk2 foreign key (p) references p (id));

-- probe: fk-own-index-name
-- base:
create table p (id int primary key);
-- v1:
create table c (id int primary key, p int, key ip (p), constraint fk foreign key (p) references p (id));
-- v2:
create table c (id int primary key, p int, key iq (p), constraint fk foreign key (p) references p (id));

-- ===== views
-- probe: view-added
-- v2:
create view v as select 1 as a;

-- probe: view-query
-- v1:
create view v as select 1 as a;
-- v2:
create view v as select 2 as a;

-- probe: view-algorithm
-- v1:
create view v as select 1 as a;
-- v2:
create algorithm = temptable view v as select 1 as a;

-- probe: view-algorithm-merge
-- base:
create table t (id int primary key, a int);
-- v1:
create algorithm = undefined view v as select * from t;
-- v2:
create algorithm = merge view v as select * from t;

-- probe: view-security
-- v1:
create view v as select 1 as a;
-- v2:
create sql security invoker view v as select 1 as a;

-- probe: view-check-option
-- base:
create table t (id int primary key, a int);
-- v1:
create view v as select * from t where a > 0;
-- v2:
create view v as select * from t where a > 0 with local check option;

-- probe: view-definer
-- v1:
create view v as select 1 as a;
-- v2:
create definer = 'gap_u1'@'%' view v as select 1 as a;

-- probe: view-column-names
-- v1:
create view v (a) as select 1;
-- v2:
create view v (b) as select 1;

-- ===== routines
-- probe: procedure-added
-- v2:
create procedure p() select 1;

-- probe: procedure-body
-- v1:
create procedure p() select 1;
-- v2:
create procedure p() select 2;

-- probe: procedure-comment
-- v1:
create procedure p() select 1;
-- v2:
create procedure p() comment 'one' select 1;

-- probe: procedure-sql-security
-- v1:
create procedure p() select 1;
-- v2:
create procedure p() sql security invoker select 1;

-- probe: procedure-data-access
-- v1:
create procedure p() select 1;
-- v2:
create procedure p() reads sql data select 1;

-- probe: procedure-definer
-- v1:
create procedure p() select 1;
-- v2:
create definer = 'gap_u1'@'%' procedure p() select 1;

-- probe: procedure-parameter-charset
-- v1:
create procedure p(x varchar(10)) select x;
-- v2:
create procedure p(x varchar(10) charset latin1) select x;

-- probe: procedure-sql-mode
-- v1:
create procedure p() select 1;
-- v2:
set session sql_mode = 'ANSI_QUOTES';
create procedure p() select 1;

-- probe: function-added
-- v2:
create function f() returns int deterministic return 1;

-- probe: function-deterministic
-- v1:
create function f() returns int no sql return 1;
-- v2:
create function f() returns int deterministic no sql return 1;

-- probe: function-comment
-- v1:
create function f() returns int deterministic return 1;
-- v2:
create function f() returns int deterministic comment 'one' return 1;

-- probe: function-return-charset
-- v1:
create function f() returns varchar(10) deterministic return 'a';
-- v2:
create function f() returns varchar(10) charset latin1 deterministic return 'a';

-- probe: function-return-collate
-- v1:
create function f() returns varchar(10) deterministic return 'a';
-- v2:
create function f() returns varchar(10) collate utf8mb4_bin deterministic return 'a';

-- probe: function-sql-mode
-- v1:
create function f() returns int deterministic return 1;
-- v2:
set session sql_mode = 'PIPES_AS_CONCAT';
create function f() returns int deterministic return 1;

-- ===== triggers
-- probe: trigger-added
-- base:
create table t (id int primary key, a int);
-- v2:
create trigger tg before insert on t for each row set new.a = 1;

-- probe: trigger-body
-- base:
create table t (id int primary key, a int);
-- v1:
create trigger tg before insert on t for each row set new.a = 1;
-- v2:
create trigger tg before insert on t for each row set new.a = 2;

-- probe: trigger-timing
-- base:
create table t (id int primary key, a int);
create table log (id int);
-- v1:
create trigger tg before insert on t for each row insert into log values (new.id);
-- v2:
create trigger tg after insert on t for each row insert into log values (new.id);

-- probe: trigger-order
-- base:
create table t (id int primary key, a int);
-- v1:
create trigger t1 before insert on t for each row set new.a = 1;
create trigger t2 before insert on t for each row set new.a = 2;
-- v2:
create trigger t2 before insert on t for each row set new.a = 2;
create trigger t1 before insert on t for each row set new.a = 1;

-- probe: trigger-precedes
-- base:
create table t (id int primary key, a int);
create trigger t1 before insert on t for each row set new.a = 1;
-- v1:
create trigger t2 before insert on t for each row set new.a = 2;
-- v2:
create trigger t2 before insert on t for each row precedes t1 set new.a = 2;

-- probe: trigger-definer
-- base:
create table t (id int primary key, a int);
-- v1:
create trigger tg before insert on t for each row set new.a = 1;
-- v2:
create definer = 'gap_u1'@'%' trigger tg before insert on t for each row set new.a = 1;

-- probe: trigger-sql-mode
-- base:
create table t (id int primary key, a int);
-- v1:
create trigger tg before insert on t for each row set new.a = 1;
-- v2:
set session sql_mode = 'ANSI_QUOTES';
create trigger tg before insert on t for each row set new.a = 1;

-- ===== events
-- probe: event-added
-- v2:
create event e on schedule every 1 day do select 1;

-- probe: event-interval
-- v1:
create event e on schedule every 1 day do select 1;
-- v2:
create event e on schedule every 2 day do select 1;

-- probe: event-ends
-- v1:
create event e on schedule every 1 day do select 1;
-- v2:
create event e on schedule every 1 day ends '2099-01-01 00:00:00' do select 1;

-- probe: event-starts
-- v1:
create event e on schedule every 1 day do select 1;
-- v2:
create event e on schedule every 1 day starts '2099-01-01 00:00:00' do select 1;

-- probe: event-disabled
-- v1:
create event e on schedule every 1 day do select 1;
-- v2:
create event e on schedule every 1 day disable do select 1;

-- probe: event-preserve
-- v1:
create event e on schedule every 1 day do select 1;
-- v2:
create event e on schedule every 1 day on completion preserve do select 1;

-- probe: event-comment
-- v1:
create event e on schedule every 1 day do select 1;
-- v2:
create event e on schedule every 1 day comment 'daily' do select 1;

-- probe: event-one-time
-- v2:
create event e on schedule at '2099-01-01 00:00:00' do select 1;

-- probe: event-definer
-- v1:
create event e on schedule every 1 day do select 1;
-- v2:
create definer = 'gap_u1'@'%' event e on schedule every 1 day do select 1;

-- probe: event-sql-mode
-- v1:
create event e on schedule every 1 day do select 1;
-- v2:
set session sql_mode = 'ANSI_QUOTES';
create event e on schedule every 1 day do select 1;

-- ===== partitions
-- probe: partition-added
-- v1:
create table t (id int primary key);
-- v2:
create table t (id int primary key) partition by hash (id) partitions 4;

-- probe: partition-count
-- v1:
create table t (id int primary key) partition by hash (id) partitions 4;
-- v2:
create table t (id int primary key) partition by hash (id) partitions 8;

-- probe: partition-linear
-- v1:
create table t (id int primary key) partition by hash (id) partitions 4;
-- v2:
create table t (id int primary key) partition by linear hash (id) partitions 4;

-- probe: partition-key-algorithm
-- v1:
create table t (id int primary key) partition by key (id) partitions 4;
-- v2:
create table t (id int primary key) partition by key algorithm = 1 (id) partitions 4;

-- probe: partition-range-bounds
-- v1:
create table t (id int primary key) partition by range (id) (partition p0 values less than (10), partition p1 values less than maxvalue);
-- v2:
create table t (id int primary key) partition by range (id) (partition p0 values less than (20), partition p1 values less than maxvalue);

-- probe: partition-list
-- v1:
create table t (id int primary key) partition by list (id) (partition p0 values in (1, 2));
-- v2:
create table t (id int primary key) partition by list (id) (partition p0 values in (1, 2, 3));

-- probe: partition-range-columns
-- v1:
create table t (id int, d date, primary key (id, d)) partition by range columns (d) (partition p0 values less than ('2020-01-01'), partition p1 values less than (maxvalue));
-- v2:
create table t (id int, d date, primary key (id, d)) partition by range columns (d) (partition p0 values less than ('2021-01-01'), partition p1 values less than (maxvalue));

-- probe: partition-comment
-- v1:
create table t (id int primary key) partition by range (id) (partition p0 values less than (10), partition p1 values less than maxvalue);
-- v2:
create table t (id int primary key) partition by range (id) (partition p0 values less than (10) comment = 'low', partition p1 values less than maxvalue);

-- probe: partition-max-rows
-- v1:
create table t (id int primary key) partition by range (id) (partition p0 values less than (10), partition p1 values less than maxvalue);
-- v2:
create table t (id int primary key) partition by range (id) (partition p0 values less than (10) max_rows = 100, partition p1 values less than maxvalue);

-- probe: partition-subpartitions
-- v1:
create table t (id int, a int, primary key (id, a)) partition by range (id) (partition p0 values less than (10), partition p1 values less than maxvalue);
-- v2:
create table t (id int, a int, primary key (id, a)) partition by range (id) subpartition by hash (a) subpartitions 2 (partition p0 values less than (10), partition p1 values less than maxvalue);

-- probe: partition-names
-- v1:
create table t (id int primary key) partition by range (id) (partition p0 values less than (10), partition p1 values less than maxvalue);
-- v2:
create table t (id int primary key) partition by range (id) (partition q0 values less than (10), partition q1 values less than maxvalue);

-- ===== the database itself (refused)
-- probe: database-charset
-- base:
create table t (id int primary key);
-- v2:
alter database character set latin1;

-- probe: database-read-only-statement
-- base:
create table t (id int primary key);
-- v2:
alter schema read only = 0;

-- ===== more
-- probe: column-default-null-explicit
-- v1:
create table t (id int primary key, a int);
-- v2:
create table t (id int primary key, a int default null);

-- probe: column-enum-collate
-- v1:
create table t (id int primary key, a enum('x', 'y'));
-- v2:
create table t (id int primary key, a enum('x', 'y') collate utf8mb4_bin);

-- probe: column-text-default-expression
-- v1:
create table t (id int primary key, a text);
-- v2:
create table t (id int primary key, a text default ('x'));

-- probe: column-generated-comment
-- v1:
create table t (id int primary key, a int as (id * 2));
-- v2:
create table t (id int primary key, a int as (id * 2) comment 'twice');

-- probe: check-auto-named
-- v1:
create table t (id int primary key, a int check (a > 0));
-- v2:
create table t (id int primary key, a int, constraint t_chk_9 check (a > 0));

-- probe: procedure-out-param
-- v1:
create procedure p(in x int) select x;
-- v2:
create procedure p(out x int) set x = 1;

-- probe: function-sql-security
-- v1:
create function f() returns int deterministic return 1;
-- v2:
create function f() returns int deterministic sql security invoker return 1;

-- probe: event-disable-on-replica
-- v1:
create event e on schedule every 1 day do select 1;
-- v2:
create event e on schedule every 1 day disable on replica do select 1;

-- probe: table-engine-archive
-- v1:
create table t (id int) engine = MyISAM;
-- v2:
create table t (id int) engine = ARCHIVE;

-- probe: table-engine-blackhole
-- v1:
create table t (id int) engine = InnoDB;
-- v2:
create table t (id int) engine = BLACKHOLE;

-- probe: partition-list-columns
-- v1:
create table t (id int, c varchar(10), primary key (id, c)) partition by list columns (c) (partition p0 values in ('a'));
-- v2:
create table t (id int, c varchar(10), primary key (id, c)) partition by list columns (c) (partition p0 values in ('a', 'b'));

-- probe: partition-engine
-- v1:
create table t (id int primary key) engine = InnoDB partition by hash (id) partitions 2;
-- v2:
create table t (id int primary key) engine = InnoDB partition by hash (id) (partition a engine = InnoDB, partition b engine = InnoDB);

-- probe: partition-subpartition-names
-- v1:
create table t (id int, a int, primary key (id, a)) partition by range (id) subpartition by hash (a) (partition p0 values less than (10) (subpartition s0, subpartition s1));
-- v2:
create table t (id int, a int, primary key (id, a)) partition by range (id) subpartition by hash (a) (partition p0 values less than (10) (subpartition s2, subpartition s3));

-- probe: index-on-generated
-- base:
create table t (id int primary key, a int as (id * 2) stored);
-- v2:
create index ia on t (a);

-- probe: column-json-check
-- v1:
create table t (id int primary key, a json);
-- v2:
create table t (id int primary key, a json check (json_valid(a)));

-- probe: view-updatable-cascaded
-- base:
create table t (id int primary key, a int);
-- v1:
create view v as select * from t where a > 0 with local check option;
-- v2:
create view v as select * from t where a > 0 with cascaded check option;

-- probe: trigger-follows
-- base:
create table t (id int primary key, a int);
create trigger t1 before insert on t for each row set new.a = 1;
create trigger t3 before insert on t for each row set new.a = 3;
-- v1:
create trigger t2 before insert on t for each row set new.a = 2;
-- v2:
create trigger t2 before insert on t for each row follows t1 set new.a = 2;

-- probe: table-encryption-n
-- v1:
create table t (id int primary key);
-- v2:
create table t (id int primary key) encryption = 'N';

-- probe: table-tablespace-file-per-table
-- v1:
create table t (id int primary key);
-- v2:
create table t (id int primary key) tablespace = innodb_file_per_table;

-- probe: column-generated-not-null
-- v1:
create table t (id int primary key, a int as (id * 2));
-- v2:
create table t (id int primary key, a int as (id * 2) not null);

-- probe: table-auto-increment-new-with-rows-elsewhere
-- base:
create table s (id int primary key);
-- v1:
create table t (id int auto_increment primary key);
-- v2:
create table t (id int auto_increment primary key) auto_increment = 500;
