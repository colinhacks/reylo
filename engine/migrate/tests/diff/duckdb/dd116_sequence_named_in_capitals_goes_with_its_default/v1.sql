create sequence "Seq"; create table t (id bigint primary key default nextval('"Seq"'), n integer);
