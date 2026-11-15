drop macro table b.x;
create macro b.x (a) as table select (a + 1) as v;
