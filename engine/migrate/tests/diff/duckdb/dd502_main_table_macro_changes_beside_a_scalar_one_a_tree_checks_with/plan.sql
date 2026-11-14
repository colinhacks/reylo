drop macro table x;
create macro x (a) as table select (a + 1) as v;
