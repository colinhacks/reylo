-- A key pins a row only under its own equality, over values that do not change from row to row
-- (the soundness review of 2026-10-04): a unique index over an expression under another collation
-- than the expression's, a timestamp key compared with a timestamptz, a user's volatile operator,
-- a column merged by USING, a WITH query named like a table read with its schema, and a column
-- pinned under an equality that is not identity.
create collation ci1 (provider = icu, locale = 'und-u-ks-level1', deterministic = false);
create table emails (id int8 primary key, email text collate ci1);
create unique index emails_lower on emails ((lower(email)) collate "C");
insert into emails values (1, 'a'), (2, 'á');
-- 02:30 does not exist on 2026-03-08 in New York, and converts to the instant of 03:30.
set timezone = 'America/New_York';
create table ev (id int8 primary key, ts timestamp unique, tstz timestamptz unique);
insert into ev values (1, '2026-03-08 02:30', '2026-03-08 03:30'), (2, '2026-03-08 03:30', '2026-03-09 00:00');
create sequence s;
create function vr(int4, int4) returns int4 language sql volatile as $$ select nextval('s')::int4 $$;
create operator +? (leftarg = int4, rightarg = int4, function = vr);
create table k (id int4 primary key, v text);
insert into k select g, 'x' from generate_series(1, 20) g;
-- USING (k) compares ua.k with ub.k converted to float8, which maps both ub rows to one value.
create table ua (id int8 primary key, k float8 unique);
create table ub (id int8 primary key, k numeric unique);
insert into ua values (1, 0.1);
insert into ub values (1, 0.1), (2, 0.10000000000000000001);
create table users (id int8 primary key, name text);
insert into users values (1, 'a'), (2, 'b');
-- 'a' and 'A' are equal under ci2, 1 and 1.0 under numeric's equality, and tell apart elsewhere.
create collation ci2 (provider = icu, locale = 'und-u-ks-level2', deterministic = false);
create table pv_users (id int4 primary key, email text);
create table pv_posts (id int4 primary key, tag text collate ci2, amount numeric);
insert into pv_users values (97, 'lower'), (65, 'upper'), (1, 'one'), (3, 'three');
insert into pv_posts values (1, 'a', 1), (2, 'A', 1.0);
-- A varchar key compared with a bpchar value whose type the engine does not read: the server
-- compares the column as bpchar, whose = ignores trailing blanks, so 'a', 'a ' and 'a  ' match.
create table vk (id int primary key, vc varchar(10) unique);
insert into vk values (1, 'a  '), (2, 'a '), (3, 'a');
-- A partial unique index whose predicate a WHERE meets only with a literal a cast changes:
-- 'a  '::bpchar meets a text column as 'a', which the index does not cover.
create table pidx (id int primary key, email text, status text);
create unique index pidx_active on pidx (email) where status = 'a  ';
insert into pidx values (1, 'a', 'a'), (2, 'a', 'a'), (3, 'a', 'a  ');
