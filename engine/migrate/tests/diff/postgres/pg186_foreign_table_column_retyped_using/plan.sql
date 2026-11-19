-- loses data: ft.c8 changes from text to integer, which stores text that reads as a number as that number ('007' becomes 7)
alter table ft alter column c8 type integer;
