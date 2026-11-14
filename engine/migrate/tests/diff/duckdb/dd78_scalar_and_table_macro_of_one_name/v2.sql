create macro pick(a) as a + 2; create macro pick(n) as table select n as x; comment on macro pick is 'scalar'; comment on macro table pick is 'rows';
