-- loses data: "some other foobar"
alter sequence "foobar sequence" owned by none;
drop table "some other foobar";
alter sequence "foobar sequence" owned by "some foobar"."some id";
