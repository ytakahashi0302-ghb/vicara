import { useState, useCallback } from 'react';
import { useDatabase } from './useDatabase';
import { Story } from '../types';

export function useStories() {
    const { db } = useDatabase();
    const [stories, setStories] = useState<Story[]>([]);
    const [loading, setLoading] = useState(false);

    const fetchStories = useCallback(async () => {
        if (!db) return;
        setLoading(true);
        try {
            const result = await db.select<Story[]>('SELECT * FROM stories ORDER BY created_at DESC');
            setStories(result);
        } catch (err) {
            console.error('Failed to fetch stories', err);
        } finally {
            setLoading(false);
        }
    }, [db]);

    const addStory = useCallback(async (story: Omit<Story, 'created_at' | 'updated_at'>) => {
        if (!db) return;
        try {
            await db.execute(
                'INSERT INTO stories (id, title, description, acceptance_criteria, status) VALUES ($1, $2, $3, $4, $5)',
                [story.id, story.title, story.description, story.acceptance_criteria, story.status]
            );
            await fetchStories();
        } catch (err) {
            console.error('Failed to add story', err);
        }
    }, [db, fetchStories]);

    const updateStory = useCallback(async (story: Story) => {
        if (!db) return;
        try {
            await db.execute(
                'UPDATE stories SET title = $1, description = $2, acceptance_criteria = $3, status = $4, updated_at = CURRENT_TIMESTAMP WHERE id = $5',
                [story.title, story.description, story.acceptance_criteria, story.status, story.id]
            );
            await fetchStories();
        } catch (err) {
            console.error('Failed to update story', err);
        }
    }, [db, fetchStories]);

    const deleteStory = useCallback(async (id: string) => {
        if (!db) return;
        try {
            await db.execute('DELETE FROM stories WHERE id = $1', [id]);
            await fetchStories();
        } catch (err) {
            console.error('Failed to delete story', err);
        }
    }, [db, fetchStories]);

    return {
        stories,
        loading,
        fetchStories,
        addStory,
        updateStory,
        deleteStory
    };
}
