-- A unique key bounds a read where the WHERE equates every column of it to a value that compares
-- as the column does. MySQL compares a string with a number as two doubles, a TIMESTAMP with a
-- constant by wall-clock time unless it looks the constant up in the index, and a column under
-- its own collation.
create table users (id int primary key, email varchar(40) not null unique, name varchar(20) not null, nick varchar(20) unique, tenant int not null, slug varchar(20) not null, unique key tenant_slug (tenant, slug));
create table big (id bigint primary key, u bigint unsigned unique);
create table money (amount decimal(30,20) not null unique, whole decimal(20,0) unique);
create table floats (f float unique, d double unique);
create table events (at datetime not null unique, day date unique, t time unique, y year unique, ts timestamp null unique, at6 datetime(6) unique, t6 time(6) unique);
create table kinds (kind enum('a','b','c') not null unique, flags set('x','y') unique, bits bit(8) unique, bits64 bit(64) unique, raw varbinary(8) unique, fixed binary(2) unique);
create table texts (ci varchar(20) collate utf8mb4_0900_ai_ci unique, pad varchar(20) collate utf8mb4_general_ci unique, bin varchar(20) collate utf8mb4_bin unique, cs varchar(20) collate utf8mb4_0900_as_cs unique, latin varchar(20) character set latin1 unique, c char(5) unique);
create table prefixes (name varchar(20) not null, unique key name_prefix (name(2)));
create table functional (id int primary key, email varchar(40), unique key lower_email ((lower(email))));
create table hidden (k int primary key auto_increment, id int not null, at timestamp not null, unique key id_key (id) invisible, unique key at_key (at) invisible);
create table stamped (tenant int not null, at timestamp not null, primary key (tenant, at));
create view user_names as select id, name from users;
-- A cast to a character type takes the connection's collation, as IMPLICIT as a column's, and of
-- the two the wider character set's wins: utf8mb4 over a utf8mb3 or latin1 key, which then compares
-- under the connection's collation, not its own.
create table bins (id int primary key, mb3 varchar(10) character set utf8mb3 collate utf8mb3_bin not null unique, l1 varchar(10) character set latin1 collate latin1_bin unique);
