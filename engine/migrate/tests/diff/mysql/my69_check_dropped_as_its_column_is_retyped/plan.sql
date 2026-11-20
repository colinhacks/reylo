-- loses data: t.flag changes from varchar(1) to tinyint(1), which stores text that reads as a number as that number ('007' becomes 7)
alter table t
  drop check my_enum,
  modify column flag tinyint(1);
