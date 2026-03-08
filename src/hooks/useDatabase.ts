import { useState, useEffect } from 'react';
import Database from '@tauri-apps/plugin-sql';

let dbInstance: Database | null = null;

export function useDatabase() {
    const [db, setDb] = useState<Database | null>(dbInstance);
    const [error, setError] = useState<Error | null>(null);

    useEffect(() => {
        async function loadDb() {
            if (dbInstance) {
                console.log('Database already loaded');
                return;
            }
            try {
                console.log('Attempting to load db sqlite:ai-scrum.db');
                const instance = await Database.load('sqlite:ai-scrum.db');
                console.log('Database successfully loaded', instance);
                dbInstance = instance;
                setDb(instance);
            } catch (err) {
                console.error('Failed to load database 🚨', err);
                setError(err instanceof Error ? err : new Error(String(err)));
            }
        }
        loadDb();
    }, []);

    return { db, error };
}
