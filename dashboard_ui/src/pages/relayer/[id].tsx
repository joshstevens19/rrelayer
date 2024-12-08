import { useGetRelayer } from '@/hooks/relayer';
import MainLayout from '@/layouts/MainLayout';
import RelayerApiKeys from '@/pages/relayer/components/RelayerApiKeys';
import RelayerGasSettings from '@/pages/relayer/components/RelayerGasSettings';
import RelayerTransactions from '@/pages/relayer/components/RelayerTransactions';
import LoadingComponent from '@/shared/components/Loading';
import { useRouter } from 'next/router';
import React, { useEffect, useState } from 'react';
import RelayerAllowlisted from './components/RelayerAllowlisted';
import RelayerHeader from './components/RelayerHeader';

enum RelayerTabs {
  Transactions = 'Transactions',
  ApiKeys = 'ApiKeys',
  Gas = 'Gas settings',
  Allowlist = 'Allowlist',
}

const Relayer: React.FC = () => {
  const router = useRouter();
  const { id } = router.query;

  const { getRelayer, loading, relayer } = useGetRelayer();

  const [activeButton, setActiveButton] = useState<RelayerTabs>(
    RelayerTabs.Transactions
  );

  const handleButtonClick = (buttonName: RelayerTabs) => {
    setActiveButton(buttonName);
    window.location.hash = `#${buttonName}`;
  };

  const tabToTitle = (tab: RelayerTabs) => {
    if (tab === RelayerTabs.ApiKeys) {
      return 'API Keys';
    }
    return tab;
  };

  useEffect(() => {
    const hash = window.location.hash.substr(1);
    if (hash && Object.values(RelayerTabs).includes(hash as RelayerTabs)) {
      setActiveButton(hash as RelayerTabs);
    }
    if (id) {
      getRelayer(id as string);
    } else {
      router.push('/relayers');
    }
  }, [id, getRelayer, router]);

  if (loading) {
    return <LoadingComponent></LoadingComponent>;
  }

  // if (!relayer) {
  //   router.push('/relayers');
  //   return;
  // }

  const TabButton: React.FC<{ tab: RelayerTabs }> = ({ tab }) => (
    <button
      onClick={() => handleButtonClick(tab)}
      className={`inline-flex items-center h-10 px-4 -mb-px text-center font-medium ${
        activeButton === tab
          ? 'text-blue-600 border-b-2 border-blue-600'
          : 'text-gray-500 border-b border-transparent hover:text-blue-600 hover:border-blue-600'
      } bg-white focus:outline-none transition-colors duration-300`}
    >
      <span className="text-sm sm:text-base capitalize">{tabToTitle(tab)}</span>
    </button>
  );

  return (
    <MainLayout>
      <div className="p-4 bg-white rounded-lg shadow-lg">
        {relayer && <RelayerHeader relayer={relayer}></RelayerHeader>}
        <div className="flex space-x-4 mt-4">
          {Object.values(RelayerTabs).map((tab) => (
            <TabButton key={tab} tab={tab} />
          ))}
        </div>
        <div className="mt-4">
          {relayer && activeButton === RelayerTabs.Transactions && (
            <RelayerTransactions relayerId={relayer.id}></RelayerTransactions>
          )}
          {relayer && activeButton === RelayerTabs.ApiKeys && (
            <RelayerApiKeys relayerId={relayer.id}></RelayerApiKeys>
          )}
          {relayer && activeButton === RelayerTabs.Gas && (
            <RelayerGasSettings relayer={relayer}></RelayerGasSettings>
          )}
          {relayer && activeButton === RelayerTabs.Allowlist && (
            <RelayerAllowlisted relayerId={relayer.id}></RelayerAllowlisted>
          )}
        </div>
      </div>
    </MainLayout>
  );
};

export default Relayer;
