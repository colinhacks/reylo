merge into t using src s on t.id = s.id when matched then update set note = 'm' returning old.note as before, new.note as after
