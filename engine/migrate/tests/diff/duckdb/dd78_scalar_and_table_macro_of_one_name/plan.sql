drop macro pick;
create macro pick (a) as ((a + 2));
comment on macro pick is 'scalar';
comment on macro table pick is 'rows';
