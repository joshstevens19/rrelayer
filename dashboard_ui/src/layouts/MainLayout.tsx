import SidebarLayout from '@/layouts/SidebarLayout';
import React, { ReactNode } from 'react';

type Props = {
  children?: ReactNode;
};

const MainLayout: React.FC<Props> = ({ children }) => {
  return (
    <div className="flex min-h-screen">
      <SidebarLayout />
      <div className="flex-1 p-8">{children}</div>
    </div>
  );
};

export default MainLayout;
