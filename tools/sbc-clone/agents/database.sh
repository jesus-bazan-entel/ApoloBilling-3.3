#!/usr/bin/env bash
# Agente: bases de datos — PostgreSQL (apolo_billing) y MariaDB (kamailio).
# Copia la estructura completa (tablas, particiones, vistas, funciones,
# triggers, secuencias) y los datos de configuración. No copia el histórico.

PG_DB=apolo_billing
# data NOT copied (history / runtime state); structure is always copied
PG_HISTORY=('public.cdrs*' 'public.active_calls' 'public.audit_logs' 'public.balance_transactions'
            'public.balance_reservations' 'public.monitoring_*_metrics' 'public.monitoring_kpi_*'
            'public.monitoring_alerts' 'public.routing_sync_log' 'public.routing_sip_status_log' 'public.*_audit_2*')
MY_DB=kamailio
MY_HISTORY=(acc acc_cdrs cdrs missed_calls location location_attrs dialog dialog_vars active_watchers watchers
            presentity pua rls_presentity rls_watchers silo sip_trace topos_d topos_t mohqcalls sca_subscriptions)

pg() { (cd /tmp && sudo -u postgres psql -X "$@"); }

capture_database() {
    local d=$1 t
    mkdir -p "$d/pg" "$d/my"

    # --- PostgreSQL ---
    local pgconf=/etc/postgresql/15/main
    cp -a "$pgconf/postgresql.conf" "$pgconf/pg_hba.conf" "$d/pg/"
    [[ -d $pgconf/conf.d ]] && cp -a "$pgconf/conf.d" "$d/pg/"
    (cd /tmp && sudo -u postgres pg_dumpall --roles-only) > "$d/pg/roles.sql"
    pg -Atc "select pg_get_userbyid(datdba) from pg_database where datname='$PG_DB'" > "$d/pg/owner"
    local -a excl=()
    for t in "${PG_HISTORY[@]}"; do excl+=(--exclude-table-data="$t"); done
    (cd /tmp && sudo -u postgres pg_dump -Fp --no-subscriptions "${excl[@]}" "$PG_DB") > "$d/pg/$PG_DB.sql"

    # row counts of every table whose data is copied, for validation
    local -a like=()
    for t in "${PG_HISTORY[@]}"; do t=${t#public.}; like+=("and c.relname not like '${t//\*/%}'"); done
    pg -d "$PG_DB" -Atc "select c.relname from pg_class c join pg_namespace n on n.oid=c.relnamespace
        where n.nspname='public' and c.relkind in ('r','p') and not c.relispartition ${like[*]} order by 1" |
    while read -r t; do
        echo "$t|$(pg -d "$PG_DB" -Atc "select count(*) from public.\"$t\"")"
    done > "$d/pg/counts.txt"

    # --- MariaDB ---
    cp -a /etc/mysql/mariadb.conf.d/50-server.cnf "$d/my/"
    mkdir -p "$d/my/conf.d"; cp -a /etc/mysql/conf.d/. "$d/my/conf.d/" 2>/dev/null || true
    mysql -N -e "select concat(user,'@',host) from mysql.user where user not in ('root','mariadb.sys','mysql','')" > "$d/my/users.txt"
    : > "$d/my/grants.sql"
    local uh u h
    while read -r uh; do
        u=${uh%@*}; h=${uh#*@}
        mysql -N -e "show grants for '$u'@'$h'" |
            sed -E "s/ IDENTIFIED BY PASSWORD '[^']*'//; s/ IDENTIFIED VIA [^;]*//; s/$/;/" >> "$d/my/grants.sql"
    done < "$d/my/users.txt"
    mysql -N -e "show create database $MY_DB" | cut -f2- > "$d/my/create-db.sql"
    local -a ign=()
    for t in "${MY_HISTORY[@]}"; do ign+=(--ignore-table-data="$MY_DB.$t"); done
    mysqldump --single-transaction --routines --triggers --events "${ign[@]}" "$MY_DB" |
        sed -E 's/ DEFINER=`[^`]+`@`[^`]+`//g' > "$d/my/$MY_DB.sql"
    mysql -N -e "select table_name from information_schema.tables where table_schema='$MY_DB' and table_type='BASE TABLE' order by 1" |
    while read -r t; do
        printf '%s\n' "${MY_HISTORY[@]}" | grep -qxF "$t" && continue
        echo "$t|$(mysql -N -e "select count(*) from \`$MY_DB\`.\`$t\`")"
    done > "$d/my/counts.txt"
    return 0
}

apply_database() {
    local d=$1 p u pass

    # --- PostgreSQL ---
    local pgconf=/etc/postgresql/15/main
    [[ -d $pgconf ]] || { log_err "PostgreSQL 15 no está instalado"; return 1; }
    cp "$d/pg/postgresql.conf" "$d/pg/pg_hba.conf" "$pgconf/"
    [[ -d $d/pg/conf.d ]] && install_tree "$d/pg/conf.d" "$pgconf/conf.d/"
    chown -R postgres:postgres "$pgconf"
    systemctl restart postgresql
    log_ok "PostgreSQL configurado como el origen"

    pg -q -c "drop database if exists $PG_DB" >> "$LOG_FILE" 2>&1
    # roles (the 'postgres' role already exists; that error is expected)
    pg -q < "$d/pg/roles.sql" >> "$LOG_FILE" 2>&1 || true
    for p in $PG_ROLE_PASSWORDS; do
        u=${p%%:*}; pass=${p#*:}
        pg -q -c "alter role \"$u\" with login password '$pass'" >> "$LOG_FILE"
    done
    pg -q -c "create database $PG_DB owner \"$(cat "$d/pg/owner")\"" >> "$LOG_FILE"
    rewrite_ips "$d/pg/$PG_DB.sql"
    pg -q -v ON_ERROR_STOP=1 -d "$PG_DB" < "$d/pg/$PG_DB.sql" >> "$LOG_FILE" 2>&1
    log_ok "PostgreSQL $PG_DB: estructura + configuración importadas"

    # new password for the portal administrator (argon2id, as the backend expects)
    local hash
    hash=$(ADMIN_PASS="$ADMIN_PASS" python3 -c 'import os; from argon2 import PasswordHasher; print(PasswordHasher().hash(os.environ["ADMIN_PASS"]))')
    # psql only interpolates :'vars' in scripts read from stdin, not in -c
    pg -q -d "$PG_DB" -v u="$ADMIN_USER" -v h="$hash" >> "$LOG_FILE" \
        <<<"update usuarios set password = :'h', activo = true where username = :'u';"
    [[ $(pg -d "$PG_DB" -At -v u="$ADMIN_USER" <<<"select count(*) from usuarios where username = :'u';") == 1 ]] ||
        { log_err "El usuario $ADMIN_USER no existe en usuarios"; return 1; }
    log_ok "Contraseña nueva para el administrador del portal ($ADMIN_USER)"

    # --- MariaDB ---
    cp "$d/my/50-server.cnf" /etc/mysql/mariadb.conf.d/50-server.cnf
    install_tree "$d/my/conf.d" /etc/mysql/conf.d 2>/dev/null || true
    systemctl restart mariadb
    mysql -e "drop database if exists $MY_DB; $(cat "$d/my/create-db.sql");"
    local uh h
    while read -r uh; do
        u=${uh%@*}; h=${uh#*@}
        pass=$(cred_of "$u" "$MY_USER_PASSWORDS")
        mysql -e "drop user if exists '$u'@'$h'; create user '$u'@'$h' identified by '$pass';"
    done < "$d/my/users.txt"
    mysql < "$d/my/grants.sql"
    mysql -e "flush privileges"
    rewrite_ips "$d/my/$MY_DB.sql"
    mysql "$MY_DB" < "$d/my/$MY_DB.sql"
    log_ok "MariaDB $MY_DB: estructura + configuración importadas, usuarios con contraseña nueva"
    return 0
}

pg_login() { PGPASSWORD=$2 psql -X -h 127.0.0.1 -U "$1" -d "$PG_DB" -Atc 'select 1' | grep -qx 1; }
my_login() { mysql -h 127.0.0.1 -u "$1" -p"$2" -N -e 'select 1' "$MY_DB" | grep -qx 1; }
pg_count_is() { [[ $(pg -d "$PG_DB" -Atc "select count(*) from public.\"$1\"") == "$2" ]]; }
my_count_is() { [[ $(mysql -N -e "select count(*) from \`$MY_DB\`.\`$1\`") == "$2" ]]; }
admin_hash_ok() {
    local h
    h=$(pg -d "$PG_DB" -At -v u="$ADMIN_USER" <<<"select password from usuarios where username = :'u';")
    H="$h" ADMIN_PASS="$ADMIN_PASS" python3 -c 'import os; from argon2 import PasswordHasher; PasswordHasher().verify(os.environ["H"], os.environ["ADMIN_PASS"])'
}
counts_match() {
    # counts_match fn file → every table in file has the same row count
    local fn=$1 file=$2 t n bad=0
    while IFS='|' read -r t n; do "$fn" "$t" "$n" || { echo "difiere: $t (origen $n)"; bad=1; }; done < "$file"
    return $bad
}

validate_database() {
    local d=$1 p
    for p in $PG_ROLE_PASSWORDS; do check "PostgreSQL: login de ${p%%:*} con la contraseña nueva" pg_login "${p%%:*}" "${p#*:}"; done
    for p in $MY_USER_PASSWORDS; do check "MariaDB: login de ${p%%:*} con la contraseña nueva" my_login "${p%%:*}" "${p#*:}"; done
    check "PostgreSQL: $(wc -l < "$d/pg/counts.txt") tablas de configuración con el mismo número de filas que el origen" \
        counts_match pg_count_is "$d/pg/counts.txt"
    check "MariaDB: $(wc -l < "$d/my/counts.txt") tablas de configuración con el mismo número de filas que el origen" \
        counts_match my_count_is "$d/my/counts.txt"
    check "PostgreSQL: particiones de cdrs creadas" test "$(pg -d "$PG_DB" -Atc "select count(*) from pg_inherits where inhparent='public.cdrs'::regclass")" -gt 0
    check "Portal: contraseña del administrador verificada (argon2)" admin_hash_ok
    return 0
}
