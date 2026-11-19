-- loses data: p.c
-- loses data: h.c
alter publication pub set table only h (id);
alter table p drop column c;
