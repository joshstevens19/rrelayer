import { useGetUsers } from '@/hooks/users';
import MainLayout from '@/layouts/MainLayout';
import LoadingComponent from '@/shared/components/Loading';
import { useEffect } from 'react';

const Users: React.FC = () => {
  const { getUsers, items, loading, next, previous } = useGetUsers();

  useEffect(() => {
    getUsers();
  }, [getUsers]);

  if (loading) {
    return <LoadingComponent></LoadingComponent>;
  }

  return (
    <MainLayout>
      <div className="flex justify-between items-center">
        <h1 className="text-2xl font-bold">Users</h1>
        <button className="bg-blue-500 hover:bg-blue-700 text-white font-bold py-2 px-4 rounded">
          Add new user
        </button>
      </div>

      <ul role="list" className="divide-y divide-gray-100">
        {items.map((person) => (
          <li
            key={person.address}
            className="flex justify-between gap-x-6 py-5"
          >
            <div className="flex min-w-0 gap-x-4">
              <img
                className="h-12 w-12 flex-none rounded-full bg-gray-50"
                src="/logo.jpeg"
                alt=""
              />
              <div className="min-w-0 flex-auto">
                <p className="text-sm font-semibold leading-6 text-gray-900">
                  {person.address}
                </p>
                <p className="mt-1 truncate text-xs leading-5 text-gray-500">
                  {person.role}
                </p>
              </div>
            </div>
            <div className="hidden shrink-0 sm:flex sm:flex-col sm:items-end">
              <p className="text-sm leading-6 text-gray-900">Revoke</p>
            </div>
          </li>
        ))}
      </ul>
    </MainLayout>
  );
};

export default Users;
