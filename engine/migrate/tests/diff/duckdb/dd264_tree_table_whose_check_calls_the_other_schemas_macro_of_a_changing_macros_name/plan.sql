drop macro a.valid;
create macro a.valid (s) as ((s != '') and (s != '-'));
