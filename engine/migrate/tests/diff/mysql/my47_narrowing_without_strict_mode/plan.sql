-- loses data: readings.code changes from varchar(10) to varchar(3), which clamps or cuts short a value it can't hold, since this session's sql_mode isn't strict
-- loses data: readings.kind changes from enum('low','mid','high') to enum('low','mid'), which clamps or cuts short a value it can't hold, since this session's sql_mode isn't strict
-- loses data: readings.level changes from int to tinyint, which clamps or cuts short a value it can't hold, since this session's sql_mode isn't strict
-- loses data: readings.note changes from tinytext to varchar(63), which clamps or cuts short a value it can't hold, since this session's sql_mode isn't strict
-- loses data: readings.ratio changes from decimal(6,2) to decimal(4,2), which clamps or cuts short a value it can't hold, since this session's sql_mode isn't strict
alter table readings
  modify column level tinyint,
  modify column code varchar(3),
  modify column kind enum('low','mid'),
  modify column ratio decimal(4,2),
  modify column note varchar(63);
