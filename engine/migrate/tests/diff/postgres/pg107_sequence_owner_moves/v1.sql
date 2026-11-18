create sequence "foobar sequence" as bigint increment by 2 minvalue 5 maxvalue 100 start with 10 cache 5 cycle owned by none;
create table "some foobar" ("some id" bigint);
create table "some other foobar" ("some id" bigint);
alter sequence "foobar sequence" owned by "some other foobar"."some id";
