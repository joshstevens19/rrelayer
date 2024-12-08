import {
  useAuthenticate,
  UseAuthenticateOptions,
} from '@/hooks/auth/UseAuthenticate';

const Authenticate: React.FC<UseAuthenticateOptions> = ({
  onSuccess,
  onError,
}) => {
  const authenticate = useAuthenticate({
    onSuccess,
    onError,
  });

  return (
    <button
      onClick={authenticate}
      className="bg-blue-500 hover:bg-blue-700 text-white font-bold py-2 px-4 rounded mt-4"
    >
      Login
    </button>
  );
};

export default Authenticate;
