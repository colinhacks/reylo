-- MySQL's prepare says nothing of a parameter's nullability, so the engine reads it off the
-- statement by the rule of Postgres and SQLite: a parameter written
-- whole and bare, with no cast, to a column that may hold NULL (a VALUES row, the SELECT that
-- feeds an INSERT, INSERT's SET, an UPDATE's SET, ON DUPLICATE KEY UPDATE, a REPLACE) takes NULL,
-- unless it also fills a column that refuses it, and so does one cast to `nullable(…)`, through the
-- column of a WITH query that selects it too. Any other cast one is its cast's type, and neither a
-- COALESCE or an IFNULL of a parameter nor a null test (IS NULL, IS NOT NULL, ISNULL(), <=>) lets
-- a NULL through. The oracle binds NULL for each nullable parameter beside its other values. The
-- INVISIBLE column takes no value from an INSERT without a column list.
create table users (id int primary key auto_increment, name varchar(20) not null, secret varchar(20) not null default 's' invisible, email varchar(40), bio varchar(40), unique key users_email (email));
create table posts (id int primary key auto_increment, user_id int not null, title varchar(40) not null, tag varchar(20));
