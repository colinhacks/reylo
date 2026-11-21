SELECT id, id IN (1, toUInt64({extra:UInt32})) AS pinned FROM users WHERE created >= toDateTime({since:String})
