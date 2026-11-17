merge into t using src s on t.id = s.id when matched then update set note = 'm' when not matched then insert (id, note) values (s.id, 'i') returning t.note, merge_action() as action
