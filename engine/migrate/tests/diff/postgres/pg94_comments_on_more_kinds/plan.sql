alter table accounts disable rule prevent_negative;
comment on collation ci is 'case-insensitive';
comment on publication audit_feed is 'audit';
comment on rule prevent_negative on accounts is 'no negative balances';
comment on sequence accounts_id_seq is 'the serial''s own';
comment on sequence ticket_seq is 'tickets';
