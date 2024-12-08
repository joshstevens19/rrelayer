import MainLayout from '@/layouts/MainLayout';
import LoadingComponent from '@/shared/components/Loading';
import { useRouter } from 'next/router';
import React, { useEffect } from 'react';

const Home: React.FC = () => {
  const router = useRouter();

  useEffect(() => {
    router.push('/login');
  }, [router]);

  return (
    <MainLayout>
      <LoadingComponent></LoadingComponent>
    </MainLayout>
  );
};

export default Home;
