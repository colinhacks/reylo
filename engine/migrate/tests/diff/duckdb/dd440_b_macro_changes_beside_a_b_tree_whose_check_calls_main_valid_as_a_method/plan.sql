drop macro b.valid;
create macro b.valid (s) as (length(s) > 6);
