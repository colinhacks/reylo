-- One probe per property SQLite's DDL can set: two schemas that differ in that property alone,
-- each planned both ways by tests/second_opinion.rs.

-- ===== tables
-- probe: table-strict
-- v1:
create table t (id integer primary key, a int);
-- v2:
create table t (id integer primary key, a int) strict;

-- probe: table-without-rowid
-- v1:
create table t (id int primary key, a int);
-- v2:
create table t (id int primary key, a int) without rowid;

-- probe: table-autoincrement
-- v1:
create table t (id integer primary key, a int);
-- v2:
create table t (id integer primary key autoincrement, a int);

-- probe: table-name-case
-- v1:
create table Users (id integer primary key);
-- v2:
create table users (id integer primary key);

-- probe: table-name-unicode-case
-- v1:
create table "É" (id integer primary key);
-- v2:
create table "é" (id integer primary key);

-- probe: table-virtual-module
-- v1:
create virtual table t using fts4(a, b);
-- v2:
create virtual table t using fts5(a, b);

-- probe: table-virtual-vs-plain
-- v1:
create table t (a, b);
-- v2:
create virtual table t using fts5(a, b);

-- probe: table-fts5-options
-- v1:
create virtual table t using fts5(a, b);
-- v2:
create virtual table t using fts5(a, b, tokenize = 'porter');

-- probe: table-fts5-unindexed
-- v1:
create virtual table t using fts5(a, b);
-- v2:
create virtual table t using fts5(a, b unindexed);

-- probe: table-rtree
-- v1:
create virtual table r using rtree(id, minx, maxx);
-- v2:
create virtual table r using rtree_i32(id, minx, maxx);

-- probe: table-rtree-aux
-- v1:
create virtual table r using rtree(id, minx, maxx);
-- v2:
create virtual table r using rtree(id, minx, maxx, +label);

-- ===== columns
-- probe: column-added
-- base:
create table t (id integer primary key);
-- v2:
alter table t add column a int;

-- probe: column-type
-- v1:
create table t (id integer primary key, a int);
-- v2:
create table t (id integer primary key, a text);

-- probe: column-type-spelling
-- v1:
create table t (id integer primary key, a varchar(10));
-- v2:
create table t (id integer primary key, a varchar(20));

-- probe: column-type-none
-- v1:
create table t (id integer primary key, a);
-- v2:
create table t (id integer primary key, a blob);

-- probe: column-not-null
-- v1:
create table t (id integer primary key, a int);
-- v2:
create table t (id integer primary key, a int not null);

-- probe: column-not-null-conflict
-- v1:
create table t (id integer primary key, a int not null);
-- v2:
create table t (id integer primary key, a int not null on conflict replace default 0);

-- probe: column-default
-- v1:
create table t (id integer primary key, a int);
-- v2:
create table t (id integer primary key, a int default 5);

-- probe: column-default-expression
-- v1:
create table t (id integer primary key, a text default 'x');
-- v2:
create table t (id integer primary key, a text default (lower('X')));

-- probe: column-default-string-case
-- v1:
create table t (id integer primary key, a text default 'Hello');
-- v2:
create table t (id integer primary key, a text default 'hello');

-- probe: column-default-dqs-case
-- v1:
create table t (id integer primary key, a text default "Hello");
-- v2:
create table t (id integer primary key, a text default "hello");

-- probe: column-check-dqs-case
-- v1:
create table t (id integer primary key, a text check (a <> "Bad"));
-- v2:
create table t (id integer primary key, a text check (a <> "bad"));

-- probe: column-collate
-- v1:
create table t (id integer primary key, a text);
-- v2:
create table t (id integer primary key, a text collate nocase);

-- probe: column-collate-rtrim
-- v1:
create table t (id integer primary key, a text collate nocase);
-- v2:
create table t (id integer primary key, a text collate rtrim);

-- probe: column-generated-virtual
-- v1:
create table t (id integer primary key, a int as (id * 2));
-- v2:
create table t (id integer primary key, a int as (id * 3));

-- probe: column-generated-stored
-- v1:
create table t (id integer primary key, a int as (id * 2) virtual);
-- v2:
create table t (id integer primary key, a int as (id * 2) stored);

-- probe: column-unique
-- v1:
create table t (id integer primary key, a int);
-- v2:
create table t (id integer primary key, a int unique);

-- probe: column-unique-conflict
-- v1:
create table t (id integer primary key, a int unique);
-- v2:
create table t (id integer primary key, a int unique on conflict ignore);

-- probe: column-name-case
-- v1:
create table t (id integer primary key, name text);
-- v2:
create table t (id integer primary key, Name text);

-- probe: column-name-unicode-case
-- v1:
create table t (id integer primary key, "É" text);
-- v2:
create table t (id integer primary key, "é" text);

-- probe: column-hidden-name-quoted
-- v1:
create table t (id integer primary key, "a b" text);
-- v2:
create table t (id integer primary key, "a c" text);

-- ===== keys and constraints
-- probe: primary-key-desc
-- v1:
create table t (id integer primary key asc, a int);
-- v2:
create table t (id integer primary key desc, a int);

-- probe: primary-key-conflict
-- v1:
create table t (id integer primary key, a int);
-- v2:
create table t (id integer primary key on conflict replace, a int);

-- probe: primary-key-composite
-- v1:
create table t (a int, b int, primary key (a));
-- v2:
create table t (a int, b int, primary key (a, b));

-- probe: check-added
-- v1:
create table t (id integer primary key, a int);
-- v2:
create table t (id integer primary key, a int, check (a > 0));

-- probe: check-named
-- v1:
create table t (id integer primary key, a int, constraint c1 check (a > 0));
-- v2:
create table t (id integer primary key, a int, constraint c2 check (a > 0));

-- probe: check-changed
-- v1:
create table t (id integer primary key, a int check (a > 0));
-- v2:
create table t (id integer primary key, a int check (a > 1));

-- probe: unique-table
-- v1:
create table t (id integer primary key, a int, b int, unique (a));
-- v2:
create table t (id integer primary key, a int, b int, unique (a, b));

-- probe: fk-added
-- base:
create table p (id integer primary key);
-- v1:
create table c (id integer primary key, p int);
-- v2:
create table c (id integer primary key, p int references p (id));

-- probe: fk-on-delete
-- base:
create table p (id integer primary key);
-- v1:
create table c (id integer primary key, p int references p (id));
-- v2:
create table c (id integer primary key, p int references p (id) on delete cascade);

-- probe: fk-on-update
-- base:
create table p (id integer primary key);
-- v1:
create table c (id integer primary key, p int references p (id));
-- v2:
create table c (id integer primary key, p int references p (id) on update set null);

-- probe: fk-deferrable
-- base:
create table p (id integer primary key);
-- v1:
create table c (id integer primary key, p int references p (id));
-- v2:
create table c (id integer primary key, p int references p (id) deferrable initially deferred);

-- probe: fk-match
-- base:
create table p (id integer primary key);
-- v1:
create table c (id integer primary key, p int references p (id));
-- v2:
create table c (id integer primary key, p int references p (id) match full);

-- probe: fk-implicit-column
-- base:
create table p (id integer primary key, k int unique);
-- v1:
create table c (id integer primary key, p int references p);
-- v2:
create table c (id integer primary key, p int references p (k));

-- probe: fk-named
-- base:
create table p (id integer primary key);
-- v1:
create table c (id integer primary key, p int, constraint f1 foreign key (p) references p (id));
-- v2:
create table c (id integer primary key, p int, constraint f2 foreign key (p) references p (id));

-- ===== indexes
-- probe: index-added
-- base:
create table t (id integer primary key, a int, b text);
-- v2:
create index ia on t (a);

-- probe: index-unique
-- base:
create table t (id integer primary key, a int, b text);
-- v1:
create index ia on t (a);
-- v2:
create unique index ia on t (a);

-- probe: index-partial
-- base:
create table t (id integer primary key, a int, b text);
-- v1:
create index ia on t (a);
-- v2:
create index ia on t (a) where a > 0;

-- probe: index-expression
-- base:
create table t (id integer primary key, a int, b text);
-- v1:
create index ie on t (lower(b));
-- v2:
create index ie on t (upper(b));

-- probe: index-collate
-- base:
create table t (id integer primary key, a int, b text);
-- v1:
create index ib on t (b);
-- v2:
create index ib on t (b collate nocase);

-- probe: index-desc
-- base:
create table t (id integer primary key, a int, b text);
-- v1:
create index ia on t (a);
-- v2:
create index ia on t (a desc);

-- probe: index-column-order
-- base:
create table t (id integer primary key, a int, b text);
-- v1:
create index iab on t (a, b);
-- v2:
create index iab on t (b, a);

-- ===== triggers and views
-- probe: trigger-added
-- base:
create table t (id integer primary key, a int);
create table log (id int);
-- v2:
create trigger tg after insert on t begin insert into log values (new.id); end;

-- probe: trigger-timing
-- base:
create table t (id integer primary key, a int);
create table log (id int);
-- v1:
create trigger tg after insert on t begin insert into log values (new.id); end;
-- v2:
create trigger tg before insert on t begin insert into log values (new.id); end;

-- probe: trigger-update-of
-- base:
create table t (id integer primary key, a int);
create table log (id int);
-- v1:
create trigger tg after update on t begin insert into log values (new.id); end;
-- v2:
create trigger tg after update of a on t begin insert into log values (new.id); end;

-- probe: trigger-when
-- base:
create table t (id integer primary key, a int);
create table log (id int);
-- v1:
create trigger tg after insert on t begin insert into log values (new.id); end;
-- v2:
create trigger tg after insert on t when new.a > 0 begin insert into log values (new.id); end;

-- probe: trigger-body-string-case
-- base:
create table t (id integer primary key, a text);
create table log (id int, m text);
-- v1:
create trigger tg after insert on t begin insert into log values (new.id, 'Added'); end;
-- v2:
create trigger tg after insert on t begin insert into log values (new.id, 'added'); end;

-- probe: trigger-instead-of
-- base:
create table t (id integer primary key, a int);
create view v as select * from t;
-- v2:
create trigger tg instead of insert on v begin insert into t values (new.id, new.a); end;

-- probe: view-added
-- base:
create table t (id integer primary key, a int);
-- v2:
create view v as select id from t;

-- probe: view-query
-- base:
create table t (id integer primary key, a int);
-- v1:
create view v as select id from t;
-- v2:
create view v as select a from t;

-- probe: view-column-names
-- base:
create table t (id integer primary key, a int);
-- v1:
create view v (x) as select id from t;
-- v2:
create view v (y) as select id from t;

-- ===== persistent settings of the file
-- probe: pragma-user-version
-- base:
create table t (id integer primary key);
-- v2:
pragma user_version = 7;

-- probe: pragma-application-id
-- base:
create table t (id integer primary key);
-- v2:
pragma application_id = 1234;

-- probe: pragma-auto-vacuum
-- v1:
create table t (id integer primary key);
-- v2:
pragma auto_vacuum = incremental;
create table t (id integer primary key);

-- probe: pragma-page-size
-- v1:
create table t (id integer primary key);
-- v2:
pragma page_size = 8192;
create table t (id integer primary key);

-- probe: analyze-stats
-- base:
create table t (id integer primary key, a int);
create index ia on t (a);
-- v2:
analyze;

-- ===== more
-- probe: pragma-encoding
-- v1:
create table t (id integer primary key);
-- v2:
pragma encoding = 'UTF-16le';
create table t (id integer primary key);

-- probe: fk-set-default
-- base:
create table p (id integer primary key);
-- v1:
create table c (id integer primary key, p int default 0 references p (id) on delete restrict);
-- v2:
create table c (id integer primary key, p int default 0 references p (id) on delete set default);

-- probe: fk-no-action-vs-restrict
-- base:
create table p (id integer primary key);
-- v1:
create table c (id integer primary key, p int references p (id) on delete no action);
-- v2:
create table c (id integer primary key, p int references p (id) on delete restrict);

-- probe: strict-without-rowid
-- v1:
create table t (id int primary key, a int) strict;
-- v2:
create table t (id int primary key, a int) strict, without rowid;

-- probe: rowid-alias
-- v1:
create table t (id int primary key, a int);
-- v2:
create table t (id integer primary key, a int);

-- probe: table-as-select
-- base:
create table s (id integer primary key, a text);
-- v1:
create table t (id int, a text);
-- v2:
create table t as select * from s;

-- probe: fts5-external-content
-- base:
create table docs (id integer primary key, body text);
-- v1:
create virtual table f using fts5(body);
-- v2:
create virtual table f using fts5(body, content = 'docs', content_rowid = 'id');

-- probe: unique-index-vs-constraint
-- v1:
create table t (id integer primary key, a int unique);
-- v2:
create table t (id integer primary key, a int);
create unique index ua on t (a);

-- probe: check-on-table-vs-column
-- v1:
create table t (id integer primary key, a int check (a > 0));
-- v2:
create table t (id integer primary key, a int, check (a > 0));

-- probe: column-default-current-timestamp
-- v1:
create table t (id integer primary key, a text default current_timestamp);
-- v2:
create table t (id integer primary key, a text default current_date);

-- probe: column-type-case
-- v1:
create table t (id integer primary key, a text);
-- v2:
create table t (id integer primary key, a TEXT);
