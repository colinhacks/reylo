merge into t using src s on t.id = s.id when matched then update set note = 'm' when not matched by source then update set n = 1 returning t.note
