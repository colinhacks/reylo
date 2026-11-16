create table events (
  id bigint not null,
  payload text,
  constraint events_pkey primary key (id)
) using heap2;
