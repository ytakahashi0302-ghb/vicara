import { useState, useCallback } from 'react';
import { useDatabase } from './useDatabase';
import { Task } from '../types';
import toast from 'react-hot-toast';

export function useTasks() {
    const { db } = useDatabase();
    const [tasks, setTasks] = useState<Task[]>([]);
    const [loading, setLoading] = useState(false);

    const fetchTasks = useCallback(async () => {
        if (!db) return;
        setLoading(true);
        try {
            const result = await db.select<Task[]>('SELECT * FROM tasks ORDER BY created_at ASC');
            setTasks(result);
        } catch (err) {
            console.error('Failed to fetch tasks', err);
        } finally {
            setLoading(false);
        }
    }, [db]);

    const fetchTasksByStoryId = useCallback(async (storyId: string) => {
        if (!db) return [];
        try {
            return await db.select<Task[]>('SELECT * FROM tasks WHERE story_id = $1 ORDER BY created_at ASC', [storyId]);
        } catch (err) {
            console.error('Failed to fetch tasks by story id', err);
            return [];
        }
    }, [db]);

    const addTask = useCallback(async (task: Omit<Task, 'created_at' | 'updated_at'>) => {
        if (!db) return;
        try {
            await db.execute(
                'INSERT INTO tasks (id, story_id, title, description, status) VALUES ($1, $2, $3, $4, $5)',
                [task.id, task.story_id, task.title, task.description, task.status]
            );
            await fetchTasks();
        } catch (err) {
            console.error('Failed to add task', err);
        }
    }, [db, fetchTasks]);

    const updateTaskStatus = useCallback(async (taskId: string, status: Task['status']) => {
        if (!db) return;

        // 楽観的UIによるフロントエンドStateの先行更新
        let previousTask: Task | undefined;
        setTasks(prev => {
            previousTask = prev.find(t => t.id === taskId);
            return prev.map(t => t.id === taskId ? { ...t, status } : t);
        });

        try {
            await db.execute(
                'UPDATE tasks SET status = $1, updated_at = CURRENT_TIMESTAMP WHERE id = $2',
                [status, taskId]
            );
            // 成功時は再取得（fetchTasks）をスキップし、dnd-kitのフリッカー（チラつき）を防止する
        } catch (err) {
            console.error('Failed to update task status', err);
            // エラー発生時は元のStateにロールバックする
            setTasks(prev =>
                prev.map(t =>
                    t.id === taskId && previousTask ? { ...t, status: previousTask.status } : t
                )
            );
            toast.error('Failed to update task status. Changes reverted.');
        }
    }, [db]);

    const updateTask = useCallback(async (task: Task) => {
        if (!db) return;
        try {
            await db.execute(
                'UPDATE tasks SET title = $1, description = $2, status = $3, updated_at = CURRENT_TIMESTAMP WHERE id = $4',
                [task.title, task.description, task.status, task.id]
            );
            await fetchTasks();
        } catch (err) {
            console.error('Failed to update task', err);
        }
    }, [db, fetchTasks]);

    const deleteTask = useCallback(async (id: string) => {
        if (!db) return;
        try {
            await db.execute('DELETE FROM tasks WHERE id = $1', [id]);
            await fetchTasks();
        } catch (err) {
            console.error('Failed to delete task', err);
        }
    }, [db, fetchTasks]);

    return {
        tasks,
        loading,
        fetchTasks,
        fetchTasksByStoryId,
        addTask,
        updateTaskStatus,
        updateTask,
        deleteTask
    };
}
