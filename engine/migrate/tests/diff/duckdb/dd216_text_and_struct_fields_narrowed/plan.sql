-- loses data: t.x changes from varchar to boolean, which stores text that reads as a boolean as that boolean ('yes' becomes true)
-- loses data: t.y changes from struct(size enum('6"', '12"'), n double) to struct(size enum('6"', '12"'), n integer), which rounds to whole numbers, in its field n
alter table t alter column x set data type boolean;
alter table t alter column y set data type struct(size enum('6"', '12"'), n integer);
