alter table t
  modify column a char(10) default '''',
  modify column b double(6,2) default '10.12',
  modify column c mediumint default '-8388608',
  modify column d time default '15:50:33',
  modify column e float default '100';
